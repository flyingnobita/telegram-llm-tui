use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use sqlite::{Connection, State, Value};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tracing::{info, warn};

use crate::telegram::events::{ChatId, DomainEvent, MessageId, UserId};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS chats (
    chat_id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    peer_kind TEXT NOT NULL,
    last_message_id INTEGER,
    last_message_at INTEGER,
    unread_count INTEGER,
    updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS messages (
    chat_id INTEGER NOT NULL,
    message_id INTEGER NOT NULL,
    author_id INTEGER NOT NULL,
    timestamp INTEGER NOT NULL,
    edit_timestamp INTEGER,
    text TEXT NOT NULL,
    outgoing INTEGER NOT NULL,
    PRIMARY KEY (chat_id, message_id)
);
CREATE INDEX IF NOT EXISTS idx_messages_chat_id ON messages(chat_id);
CREATE INDEX IF NOT EXISTS idx_messages_chat_timestamp ON messages(chat_id, timestamp);
"#;

const MESSAGE_OVERHEAD_BYTES: usize = 64;
const CHAT_OVERHEAD_BYTES: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] sqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("cache task failed: {0}")]
    Task(String),
}

pub type Result<T> = std::result::Result<T, CacheError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatPeerKind {
    User,
    Group,
    Channel,
    Unknown,
}

impl ChatPeerKind {
    fn as_str(self) -> &'static str {
        match self {
            ChatPeerKind::User => "user",
            ChatPeerKind::Group => "group",
            ChatPeerKind::Channel => "channel",
            ChatPeerKind::Unknown => "unknown",
        }
    }

    fn from_str(raw: &str) -> Self {
        match raw {
            "user" => ChatPeerKind::User,
            "group" => ChatPeerKind::Group,
            "channel" => ChatPeerKind::Channel,
            _ => ChatPeerKind::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSummary {
    pub chat_id: ChatId,
    pub title: String,
    pub peer_kind: ChatPeerKind,
    pub last_message_id: Option<MessageId>,
    pub last_message_at: Option<i64>,
    pub unread_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedMessage {
    pub chat_id: ChatId,
    pub message_id: MessageId,
    pub author_id: UserId,
    pub timestamp: i64,
    pub edit_timestamp: Option<i64>,
    pub text: String,
    pub outgoing: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct CacheLimits {
    pub max_chats: usize,
    pub max_messages_per_chat: usize,
    pub max_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub db_path: PathBuf,
    pub limits: CacheLimits,
    pub flush_debounce: Duration,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CacheSnapshot {
    pub chats: Vec<ChatSummary>,
    pub messages: Vec<CachedMessage>,
}

pub trait CacheStore: Send + Sync {
    fn load(&self) -> Result<CacheSnapshot>;
    fn save(&self, snapshot: &CacheSnapshot) -> Result<()>;

    fn upsert_chat(&self, summary: &ChatSummary) -> Result<()> {
        let mut snapshot = self.load()?;
        if let Some(existing) = snapshot
            .chats
            .iter_mut()
            .find(|chat| chat.chat_id == summary.chat_id)
        {
            *existing = summary.clone();
        } else {
            snapshot.chats.push(summary.clone());
        }
        self.save(&snapshot)
    }

    fn upsert_message(&self, message: &CachedMessage) -> Result<()> {
        let mut snapshot = self.load()?;
        if let Some(existing) = snapshot.messages.iter_mut().find(|entry| {
            entry.chat_id == message.chat_id && entry.message_id == message.message_id
        }) {
            *existing = message.clone();
        } else {
            snapshot.messages.push(message.clone());
        }
        self.save(&snapshot)
    }

    fn delete_message(&self, chat_id: ChatId, message_id: MessageId) -> Result<()> {
        let mut snapshot = self.load()?;
        snapshot
            .messages
            .retain(|entry| !(entry.chat_id == chat_id && entry.message_id == message_id));
        self.save(&snapshot)
    }

    fn clear(&self) -> Result<()> {
        self.save(&CacheSnapshot::default())
    }
}

#[derive(Debug, Clone)]
pub struct SqliteCacheStore {
    path: PathBuf,
}

impl SqliteCacheStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn open_connection(&self) -> Result<Connection> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = sqlite::open(&self.path)?;
        connection.execute(SCHEMA)?;
        Ok(connection)
    }
}

impl CacheStore for SqliteCacheStore {
    fn load(&self) -> Result<CacheSnapshot> {
        let connection = self.open_connection()?;
        let mut chats = Vec::new();
        let mut messages = Vec::new();

        let mut chat_stmt = connection.prepare(
            "SELECT chat_id, title, peer_kind, last_message_id, last_message_at, unread_count, updated_at FROM chats",
        )?;
        while let State::Row = chat_stmt.next()? {
            let chat_id = ChatId(chat_stmt.read::<i64, _>(0)?);
            let title = chat_stmt.read::<String, _>(1)?;
            let peer_kind = ChatPeerKind::from_str(chat_stmt.read::<String, _>(2)?.as_str());
            let last_message_id = chat_stmt.read::<Option<i64>, _>(3)?;
            let last_message_at = chat_stmt.read::<Option<i64>, _>(4)?;
            let unread_count = chat_stmt.read::<Option<i64>, _>(5)?;
            let _updated_at = chat_stmt.read::<i64, _>(6)?;

            chats.push(ChatSummary {
                chat_id,
                title,
                peer_kind,
                last_message_id: last_message_id.map(MessageId),
                last_message_at,
                unread_count: unread_count.map(|value| value as u32),
            });
        }

        let mut message_stmt = connection.prepare(
            "SELECT chat_id, message_id, author_id, timestamp, edit_timestamp, text, outgoing FROM messages ORDER BY chat_id, timestamp",
        )?;
        while let State::Row = message_stmt.next()? {
            let chat_id = ChatId(message_stmt.read::<i64, _>(0)?);
            let message_id = MessageId(message_stmt.read::<i64, _>(1)?);
            let author_id = UserId(message_stmt.read::<i64, _>(2)?);
            let timestamp = message_stmt.read::<i64, _>(3)?;
            let edit_timestamp = message_stmt.read::<Option<i64>, _>(4)?;
            let text = message_stmt.read::<String, _>(5)?;
            let outgoing = message_stmt.read::<i64, _>(6)? != 0;

            messages.push(CachedMessage {
                chat_id,
                message_id,
                author_id,
                timestamp,
                edit_timestamp,
                text,
                outgoing,
            });
        }

        Ok(CacheSnapshot { chats, messages })
    }

    fn save(&self, snapshot: &CacheSnapshot) -> Result<()> {
        let connection = self.open_connection()?;
        connection.execute("BEGIN IMMEDIATE TRANSACTION")?;
        connection.execute("DELETE FROM messages")?;
        connection.execute("DELETE FROM chats")?;

        {
            let mut chat_stmt = connection.prepare(
                "INSERT INTO chats (chat_id, title, peer_kind, last_message_id, last_message_at, unread_count, updated_at) VALUES (:chat_id, :title, :peer_kind, :last_message_id, :last_message_at, :unread_count, :updated_at)",
            )?;
            for chat in &snapshot.chats {
                let updated_at = chat.last_message_at.unwrap_or(0);
                chat_stmt.bind_iter::<_, (_, Value)>([
                    (":chat_id", (chat.chat_id.0).into()),
                    (":title", chat.title.clone().into()),
                    (":peer_kind", chat.peer_kind.as_str().into()),
                    (
                        ":last_message_id",
                        chat.last_message_id.map(|id| id.0).into(),
                    ),
                    (":last_message_at", chat.last_message_at.into()),
                    (
                        ":unread_count",
                        chat.unread_count.map(|value| value as i64).into(),
                    ),
                    (":updated_at", updated_at.into()),
                ])?;
                let _ = chat_stmt.next()?;
                chat_stmt.reset()?;
            }
        }

        {
            let mut message_stmt = connection.prepare(
                "INSERT INTO messages (chat_id, message_id, author_id, timestamp, edit_timestamp, text, outgoing) VALUES (:chat_id, :message_id, :author_id, :timestamp, :edit_timestamp, :text, :outgoing)",
            )?;
            for message in &snapshot.messages {
                message_stmt.bind_iter::<_, (_, Value)>([
                    (":chat_id", (message.chat_id.0).into()),
                    (":message_id", (message.message_id.0).into()),
                    (":author_id", (message.author_id.0).into()),
                    (":timestamp", message.timestamp.into()),
                    (":edit_timestamp", message.edit_timestamp.into()),
                    (":text", message.text.clone().into()),
                    (
                        ":outgoing",
                        if message.outgoing { 1i64 } else { 0i64 }.into(),
                    ),
                ])?;
                let _ = message_stmt.next()?;
                message_stmt.reset()?;
            }
        }

        connection.execute("COMMIT")?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct CacheManager {
    inner: Arc<RwLock<ChatCache>>,
    flush_tx: mpsc::UnboundedSender<FlushCommand>,
    join: JoinHandle<()>,
}

impl CacheManager {
    pub async fn spawn(store: Arc<dyn CacheStore>, config: CacheConfig) -> Result<Self> {
        let snapshot = tokio::task::spawn_blocking({
            let store = Arc::clone(&store);
            move || store.load()
        })
        .await
        .map_err(|err| CacheError::Task(err.to_string()))??;

        let cache = ChatCache::from_snapshot(snapshot, config.limits);
        let inner = Arc::new(RwLock::new(cache));
        let (flush_tx, flush_rx) = mpsc::unbounded_channel();
        let join = spawn_flush_task(Arc::clone(&inner), store, flush_rx, config.flush_debounce);

        info!(
            chats = inner.read().map(|cache| cache.chat_count()).unwrap_or(0),
            "cache loaded"
        );

        Ok(Self {
            inner,
            flush_tx,
            join,
        })
    }

    pub fn apply_event(&self, event: &DomainEvent) {
        let mut cache = match self.inner.write() {
            Ok(cache) => cache,
            Err(poisoned) => poisoned.into_inner(),
        };
        let stats = cache.apply_event(event);
        if stats.any_evicted() {
            info!(
                chats = stats.chats_evicted,
                messages = stats.messages_evicted,
                "cache eviction applied"
            );
        }
        let _ = self.flush_tx.send(FlushCommand::Dirty);
    }

    pub fn upsert_chat(&self, summary: ChatSummary) {
        let mut cache = match self.inner.write() {
            Ok(cache) => cache,
            Err(poisoned) => poisoned.into_inner(),
        };
        let stats = cache.upsert_chat(summary);
        if stats.any_evicted() {
            info!(
                chats = stats.chats_evicted,
                messages = stats.messages_evicted,
                "cache eviction applied"
            );
        }
        let _ = self.flush_tx.send(FlushCommand::Dirty);
    }

    pub fn chat_summaries(&self) -> Vec<ChatSummary> {
        let cache = self.inner.read().map(|cache| cache.chat_summaries());
        cache.unwrap_or_default()
    }

    pub fn messages_for_chat(&self, chat_id: ChatId, limit: Option<usize>) -> Vec<CachedMessage> {
        let cache = self
            .inner
            .read()
            .map(|cache| cache.messages_for_chat(chat_id, limit));
        cache.unwrap_or_default()
    }

    pub async fn shutdown(self) {
        let _ = self.flush_tx.send(FlushCommand::Shutdown);
        let _ = self.join.await;
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EvictionStats {
    pub chats_evicted: usize,
    pub messages_evicted: usize,
}

impl EvictionStats {
    fn any_evicted(self) -> bool {
        self.chats_evicted > 0 || self.messages_evicted > 0
    }
}

#[derive(Debug)]
struct ChatEntry {
    summary: ChatSummary,
    messages: VecDeque<CachedMessage>,
    updated_at: i64,
    message_bytes: usize,
    summary_bytes: usize,
}

#[derive(Debug)]
pub struct ChatCache {
    chats: HashMap<ChatId, ChatEntry>,
    limits: CacheLimits,
    current_bytes: usize,
}

impl ChatCache {
    pub fn new(limits: CacheLimits) -> Self {
        Self {
            chats: HashMap::new(),
            limits,
            current_bytes: 0,
        }
    }

    pub fn from_snapshot(snapshot: CacheSnapshot, limits: CacheLimits) -> Self {
        let mut cache = Self::new(limits);
        for chat in snapshot.chats {
            cache.insert_chat(chat);
        }
        for message in snapshot.messages {
            cache.insert_message(message);
        }
        let _ = cache.enforce_limits();
        cache
    }

    pub fn chat_count(&self) -> usize {
        self.chats.len()
    }

    pub fn snapshot(&self) -> CacheSnapshot {
        let mut chats = Vec::with_capacity(self.chats.len());
        let mut messages = Vec::new();
        for entry in self.chats.values() {
            chats.push(entry.summary.clone());
            messages.extend(entry.messages.iter().cloned());
        }
        CacheSnapshot { chats, messages }
    }

    pub fn chat_summaries(&self) -> Vec<ChatSummary> {
        self.chats
            .values()
            .map(|entry| entry.summary.clone())
            .collect()
    }

    pub fn messages_for_chat(&self, chat_id: ChatId, limit: Option<usize>) -> Vec<CachedMessage> {
        let Some(entry) = self.chats.get(&chat_id) else {
            return Vec::new();
        };
        match limit {
            Some(limit) => {
                let mut messages = entry
                    .messages
                    .iter()
                    .rev()
                    .take(limit)
                    .cloned()
                    .collect::<Vec<_>>();
                messages.reverse();
                messages
            }
            None => entry.messages.iter().cloned().collect::<Vec<_>>(),
        }
    }

    pub fn apply_event(&mut self, event: &DomainEvent) -> EvictionStats {
        match event {
            DomainEvent::MessageNew(message) => {
                let cached = CachedMessage {
                    chat_id: message.chat_id,
                    message_id: message.message_id,
                    author_id: message.author_id,
                    timestamp: message.timestamp,
                    edit_timestamp: None,
                    text: message.text.clone(),
                    outgoing: message.outgoing,
                };
                self.insert_message(cached);
            }
            DomainEvent::MessageEdited(message) => {
                self.update_message(
                    message.chat_id,
                    message.message_id,
                    &message.text,
                    message.timestamp,
                );
            }
            DomainEvent::ReadReceipt(receipt) => {
                if let Some(entry) = self.chats.get_mut(&receipt.chat_id) {
                    entry.summary.unread_count = Some(0);
                    entry.updated_at = receipt.timestamp;
                }
            }
            DomainEvent::Typing(_) => {}
        }
        self.enforce_limits()
    }

    pub fn upsert_chat(&mut self, summary: ChatSummary) -> EvictionStats {
        self.insert_chat(summary);
        self.enforce_limits()
    }

    fn insert_chat(&mut self, summary: ChatSummary) {
        let updated_at = summary.last_message_at.unwrap_or(0);
        if let Some(entry) = self.chats.get_mut(&summary.chat_id) {
            self.current_bytes = self.current_bytes.saturating_sub(entry.summary_bytes);
            entry.summary = summary;
            entry.summary_bytes = summary_size_bytes(&entry.summary);
            entry.updated_at = updated_at;
            self.current_bytes += entry.summary_bytes;
            return;
        }

        let summary_bytes = summary_size_bytes(&summary);
        let entry = ChatEntry {
            summary,
            messages: VecDeque::new(),
            updated_at,
            message_bytes: 0,
            summary_bytes,
        };
        self.current_bytes += summary_bytes;
        self.chats.insert(entry.summary.chat_id, entry);
    }

    fn insert_message(&mut self, message: CachedMessage) {
        let entry = self.chats.entry(message.chat_id).or_insert_with(|| {
            let summary = ChatSummary {
                chat_id: message.chat_id,
                title: String::new(),
                peer_kind: ChatPeerKind::Unknown,
                last_message_id: None,
                last_message_at: None,
                unread_count: None,
            };
            let summary_bytes = summary_size_bytes(&summary);
            self.current_bytes += summary_bytes;
            ChatEntry {
                summary,
                messages: VecDeque::new(),
                updated_at: 0,
                message_bytes: 0,
                summary_bytes,
            }
        });

        if let Some(existing) = entry
            .messages
            .iter_mut()
            .find(|cached| cached.message_id == message.message_id)
        {
            let old_size = message_size_bytes(existing);
            *existing = message;
            let new_size = message_size_bytes(existing);
            entry.message_bytes = entry.message_bytes.saturating_sub(old_size) + new_size;
            self.current_bytes = self.current_bytes.saturating_sub(old_size) + new_size;
        } else {
            entry.messages.push_back(message);
            let size = message_size_bytes(entry.messages.back().expect("message added"));
            entry.message_bytes += size;
            self.current_bytes += size;
        }

        if let Some(last) = entry.messages.back() {
            entry.summary.last_message_id = Some(last.message_id);
            entry.summary.last_message_at = Some(last.timestamp);
            entry.updated_at = last.timestamp;
        }
    }

    fn update_message(
        &mut self,
        chat_id: ChatId,
        message_id: MessageId,
        text: &str,
        timestamp: i64,
    ) {
        let Some(entry) = self.chats.get_mut(&chat_id) else {
            return;
        };
        if let Some(existing) = entry
            .messages
            .iter_mut()
            .find(|cached| cached.message_id == message_id)
        {
            let old_size = message_size_bytes(existing);
            existing.text = text.to_string();
            existing.edit_timestamp = Some(timestamp);
            let new_size = message_size_bytes(existing);
            entry.message_bytes = entry.message_bytes.saturating_sub(old_size) + new_size;
            self.current_bytes = self.current_bytes.saturating_sub(old_size) + new_size;
            entry.updated_at = timestamp;
        }
    }

    fn enforce_limits(&mut self) -> EvictionStats {
        let mut stats = EvictionStats::default();
        if self.limits.max_messages_per_chat > 0 {
            for entry in self.chats.values_mut() {
                while entry.messages.len() > self.limits.max_messages_per_chat {
                    if let Some(removed) = entry.messages.pop_front() {
                        let size = message_size_bytes(&removed);
                        entry.message_bytes = entry.message_bytes.saturating_sub(size);
                        self.current_bytes = self.current_bytes.saturating_sub(size);
                        stats.messages_evicted += 1;
                    }
                }
            }
        }

        if self.limits.max_chats > 0 {
            while self.chats.len() > self.limits.max_chats {
                if let Some(chat_id) = self.least_recent_chat() {
                    self.remove_chat(chat_id, &mut stats);
                } else {
                    break;
                }
            }
        }

        if self.limits.max_bytes > 0 {
            while self.current_bytes > self.limits.max_bytes {
                if let Some(chat_id) = self.least_recent_chat() {
                    self.remove_chat(chat_id, &mut stats);
                } else {
                    break;
                }
            }
        }

        stats
    }

    fn least_recent_chat(&self) -> Option<ChatId> {
        self.chats
            .iter()
            .min_by_key(|(_, entry)| entry.updated_at)
            .map(|(chat_id, _)| *chat_id)
    }

    fn remove_chat(&mut self, chat_id: ChatId, stats: &mut EvictionStats) {
        if let Some(entry) = self.chats.remove(&chat_id) {
            stats.chats_evicted += 1;
            stats.messages_evicted += entry.messages.len();
            self.current_bytes = self
                .current_bytes
                .saturating_sub(entry.message_bytes + entry.summary_bytes);
        }
    }
}

#[derive(Debug)]
enum FlushCommand {
    Dirty,
    Shutdown,
}

fn spawn_flush_task(
    inner: Arc<RwLock<ChatCache>>,
    store: Arc<dyn CacheStore>,
    mut flush_rx: mpsc::UnboundedReceiver<FlushCommand>,
    debounce: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut dirty = false;
        let mut next_flush: Option<Instant> = None;

        loop {
            if let Some(deadline) = next_flush {
                tokio::select! {
                    cmd = flush_rx.recv() => {
                        match cmd {
                            Some(FlushCommand::Dirty) => {
                                dirty = true;
                                next_flush = Some(Instant::now() + debounce);
                            }
                            Some(FlushCommand::Shutdown) | None => {
                                if dirty {
                                    flush_snapshot(&inner, &store).await;
                                }
                                break;
                            }
                        }
                    }
                    _ = tokio::time::sleep_until(deadline) => {
                        if dirty {
                            flush_snapshot(&inner, &store).await;
                            dirty = false;
                        }
                        next_flush = None;
                    }
                }
            } else {
                match flush_rx.recv().await {
                    Some(FlushCommand::Dirty) => {
                        dirty = true;
                        next_flush = Some(Instant::now() + debounce);
                    }
                    Some(FlushCommand::Shutdown) | None => {
                        if dirty {
                            flush_snapshot(&inner, &store).await;
                        }
                        break;
                    }
                }
            }
        }
    })
}

async fn flush_snapshot(inner: &Arc<RwLock<ChatCache>>, store: &Arc<dyn CacheStore>) {
    let snapshot = match inner.read() {
        Ok(cache) => cache.snapshot(),
        Err(poisoned) => poisoned.into_inner().snapshot(),
    };

    let result = tokio::task::spawn_blocking({
        let store = Arc::clone(store);
        let snapshot = snapshot.clone();
        move || store.save(&snapshot)
    })
    .await;

    match result {
        Ok(Ok(())) => {
            info!(
                chats = snapshot.chats.len(),
                messages = snapshot.messages.len(),
                "cache flushed"
            );
        }
        Ok(Err(err)) => {
            warn!(error = %err, "cache flush failed");
        }
        Err(err) => {
            warn!(error = %err, "cache flush task failed");
        }
    }
}

fn message_size_bytes(message: &CachedMessage) -> usize {
    message.text.len().saturating_add(MESSAGE_OVERHEAD_BYTES)
}

fn summary_size_bytes(summary: &ChatSummary) -> usize {
    summary.title.len().saturating_add(CHAT_OVERHEAD_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telegram::events::{DomainEvent, MessageEdited, MessageNew, ReadReceipt};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    fn cache_limits() -> CacheLimits {
        CacheLimits {
            max_chats: 2,
            max_messages_per_chat: 3,
            max_bytes: 0,
        }
    }

    fn base_message(chat_id: i64, message_id: i64, timestamp: i64, text: &str) -> MessageNew {
        MessageNew {
            chat_id: ChatId(chat_id),
            message_id: MessageId(message_id),
            author_id: UserId(1),
            timestamp,
            text: text.to_string(),
            outgoing: false,
        }
    }

    #[test]
    fn applies_message_edit_updates_text() {
        let mut cache = ChatCache::new(cache_limits());
        let new = base_message(1, 10, 100, "hello");
        cache.apply_event(&DomainEvent::MessageNew(new));

        let edit = MessageEdited {
            chat_id: ChatId(1),
            message_id: MessageId(10),
            editor_id: UserId(1),
            timestamp: 120,
            text: "updated".to_string(),
            outgoing: false,
        };
        cache.apply_event(&DomainEvent::MessageEdited(edit));

        let messages = cache.messages_for_chat(ChatId(1), None);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "updated");
        assert_eq!(messages[0].edit_timestamp, Some(120));
    }

    #[test]
    fn evicts_oldest_messages_and_chats() {
        let mut cache = ChatCache::new(cache_limits());
        cache.apply_event(&DomainEvent::MessageNew(base_message(1, 1, 100, "one")));
        cache.apply_event(&DomainEvent::MessageNew(base_message(1, 2, 101, "two")));
        cache.apply_event(&DomainEvent::MessageNew(base_message(1, 3, 102, "three")));
        cache.apply_event(&DomainEvent::MessageNew(base_message(1, 4, 103, "four")));

        let messages = cache.messages_for_chat(ChatId(1), None);
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].message_id, MessageId(2));

        cache.apply_event(&DomainEvent::MessageNew(base_message(2, 1, 200, "second")));
        cache.apply_event(&DomainEvent::MessageNew(base_message(3, 1, 300, "third")));
        assert_eq!(cache.chat_count(), 2);
        assert!(cache.chats.contains_key(&ChatId(2)));
        assert!(cache.chats.contains_key(&ChatId(3)));
    }

    #[test]
    fn read_receipt_sets_unread_count() {
        let mut cache = ChatCache::new(cache_limits());
        cache.apply_event(&DomainEvent::MessageNew(base_message(1, 1, 100, "one")));
        cache.apply_event(&DomainEvent::ReadReceipt(ReadReceipt {
            chat_id: ChatId(1),
            reader_id: UserId(1),
            timestamp: 150,
            last_read_message_id: MessageId(1),
        }));
        let summary = cache
            .snapshot()
            .chats
            .into_iter()
            .find(|summary| summary.chat_id == ChatId(1))
            .expect("summary");
        assert_eq!(summary.unread_count, Some(0));
    }

    #[test]
    fn snapshot_round_trip_with_sqlite_store() {
        let temp_path = temp_cache_path("snapshot");
        let store = SqliteCacheStore::new(temp_path.clone());

        let snapshot = CacheSnapshot {
            chats: vec![ChatSummary {
                chat_id: ChatId(1),
                title: "Chat".to_string(),
                peer_kind: ChatPeerKind::User,
                last_message_id: Some(MessageId(2)),
                last_message_at: Some(123),
                unread_count: Some(1),
            }],
            messages: vec![CachedMessage {
                chat_id: ChatId(1),
                message_id: MessageId(2),
                author_id: UserId(1),
                timestamp: 123,
                edit_timestamp: None,
                text: "hello".to_string(),
                outgoing: true,
            }],
        };

        store.save(&snapshot).expect("save snapshot");
        let loaded = store.load().expect("load snapshot");
        assert_eq!(loaded, snapshot);

        let _ = std::fs::remove_file(temp_path);
    }

    #[tokio::test]
    async fn debounced_flush_coalesces_updates() {
        let store = Arc::new(InMemoryStore::default());
        let store_for_manager: Arc<dyn CacheStore> = store.clone();
        let config = CacheConfig {
            db_path: PathBuf::from(":memory:"),
            limits: CacheLimits {
                max_chats: 0,
                max_messages_per_chat: 10,
                max_bytes: 0,
            },
            flush_debounce: Duration::from_millis(20),
        };

        let manager = CacheManager::spawn(store_for_manager, config)
            .await
            .expect("spawn manager");

        manager.apply_event(&DomainEvent::MessageNew(base_message(1, 1, 100, "one")));
        manager.apply_event(&DomainEvent::MessageNew(base_message(1, 2, 101, "two")));

        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert_eq!(store.save_count(), 0);

        tokio::time::sleep(Duration::from_millis(40)).await;
        assert_eq!(store.save_count(), 1);

        manager.shutdown().await;
    }

    fn temp_cache_path(label: &str) -> PathBuf {
        let value = CACHE_TEST_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;
        let file_name = format!(
            "telegram-llm-cache-{}-{}-{}.sqlite",
            label,
            std::process::id(),
            value
        );
        std::env::temp_dir().join(file_name)
    }

    static CACHE_TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[derive(Default)]
    struct InMemoryStore {
        snapshots: Mutex<Vec<CacheSnapshot>>,
    }

    impl InMemoryStore {
        fn save_count(&self) -> usize {
            self.snapshots.lock().unwrap().len()
        }
    }

    impl CacheStore for InMemoryStore {
        fn load(&self) -> Result<CacheSnapshot> {
            Ok(self
                .snapshots
                .lock()
                .unwrap()
                .last()
                .cloned()
                .unwrap_or_default())
        }

        fn save(&self, snapshot: &CacheSnapshot) -> Result<()> {
            self.snapshots.lock().unwrap().push(snapshot.clone());
            Ok(())
        }
    }
}
