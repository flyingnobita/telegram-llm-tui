use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use telegram_llm_core::telegram::{CacheManager, CachedMessage, ChatId, ChatSummary, UserId};
use time::{format_description, OffsetDateTime};
use ui::view::{ChatListItem, MessageItem, UiState};

#[derive(Debug, Clone)]
pub struct UiCacheBridge {
    pub state: UiState,
    selected_chat: Option<ChatId>,
    message_limit: Option<usize>,
}

impl UiCacheBridge {
    pub fn new(message_limit: Option<usize>, chat_list_width: u16) -> Self {
        let state = UiState {
            chat_list_width,
            ..UiState::default()
        };
        Self {
            state,
            selected_chat: None,
            message_limit,
        }
    }

    #[cfg(test)]
    pub fn set_selected_chat(&mut self, chat_id: Option<ChatId>) {
        self.selected_chat = chat_id;
    }

    #[cfg(test)]
    pub fn selected_chat(&self) -> Option<ChatId> {
        self.selected_chat
    }

    pub fn sync_selected_chat_from_state(&mut self) -> bool {
        let selected = self
            .state
            .chats
            .iter()
            .find(|chat| chat.is_selected)
            .map(|chat| ChatId(chat.id));
        if selected != self.selected_chat {
            self.selected_chat = selected;
            return true;
        }
        false
    }

    pub fn refresh(&mut self, cache: &CacheManager) -> Option<ChatId> {
        let summaries = cache.chat_summaries();
        let (chat_items, selected_chat) = map_chat_summaries(&summaries, self.selected_chat);
        self.selected_chat = selected_chat;
        self.state.chats = chat_items;

        self.state.messages = match selected_chat {
            Some(chat_id) => {
                let messages = cache.messages_for_chat(chat_id, self.message_limit);
                let author_names = resolve_author_names(cache, &messages);
                map_messages(messages, &author_names)
            }
            None => Vec::new(),
        };
        self.state.message_view.reconcile(&self.state.messages);

        selected_chat
    }
}

fn map_chat_summaries(
    summaries: &[ChatSummary],
    selected_chat: Option<ChatId>,
) -> (Vec<ChatListItem>, Option<ChatId>) {
    let mut sorted = summaries.to_vec();
    sorted.sort_by(|left, right| {
        let left_ts = left.last_message_at.unwrap_or(0);
        let right_ts = right.last_message_at.unwrap_or(0);
        match right_ts.cmp(&left_ts) {
            Ordering::Equal => left.title.cmp(&right.title),
            ordering => ordering,
        }
    });

    let resolved_selection = selected_chat
        .filter(|chat_id| sorted.iter().any(|chat| chat.chat_id == *chat_id))
        .or_else(|| sorted.first().map(|chat| chat.chat_id));

    let items = sorted
        .iter()
        .map(|chat| ChatListItem {
            id: chat.chat_id.0,
            title: chat_title(chat),
            unread: chat.unread_count.unwrap_or(0),
            is_selected: resolved_selection == Some(chat.chat_id),
        })
        .collect();

    (items, resolved_selection)
}

fn chat_title(chat: &ChatSummary) -> String {
    if chat.title.trim().is_empty() {
        format!("Chat {}", chat.chat_id.0)
    } else {
        chat.title.clone()
    }
}

fn map_messages(
    mut messages: Vec<CachedMessage>,
    author_names: &HashMap<UserId, String>,
) -> Vec<MessageItem> {
    messages.sort_by_key(|message| message.timestamp);
    messages
        .into_iter()
        .map(|message| MessageItem {
            id: message.message_id.0,
            author: message_author_label(&message, author_names),
            timestamp: format_timestamp(message.timestamp),
            body: message.text,
        })
        .collect()
}

fn message_author_label(message: &CachedMessage, author_names: &HashMap<UserId, String>) -> String {
    if message.outgoing {
        "You".to_string()
    } else if let Some(name) = author_names.get(&message.author_id) {
        name.clone()
    } else {
        format!("User {}", message.author_id.0)
    }
}

fn resolve_author_names(
    cache: &CacheManager,
    messages: &[CachedMessage],
) -> HashMap<UserId, String> {
    let mut author_ids = HashSet::new();
    for message in messages {
        author_ids.insert(message.author_id);
    }
    cache.user_display_names(author_ids)
}

fn format_timestamp(timestamp: i64) -> String {
    let format = match format_description::parse("[hour]:[minute]") {
        Ok(format) => format,
        Err(_) => return timestamp.to_string(),
    };
    let date_time = match OffsetDateTime::from_unix_timestamp(timestamp) {
        Ok(date_time) => date_time,
        Err(_) => return timestamp.to_string(),
    };
    date_time
        .format(&format)
        .unwrap_or_else(|_| timestamp.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use telegram_llm_core::telegram::{
        CacheConfig, CacheError, CacheLimits, CacheSnapshot, CacheStore, CachedUser, ChatPeerKind,
        ChatSummary, DomainEvent, MessageId, MessageNew, UserId,
    };

    #[derive(Default)]
    struct InMemoryStore {
        snapshot: Mutex<CacheSnapshot>,
    }

    impl CacheStore for InMemoryStore {
        fn load(&self) -> Result<CacheSnapshot, CacheError> {
            Ok(self.snapshot.lock().unwrap().clone())
        }

        fn save(&self, snapshot: &CacheSnapshot) -> Result<(), CacheError> {
            *self.snapshot.lock().unwrap() = snapshot.clone();
            Ok(())
        }
    }

    fn cache_config() -> CacheConfig {
        CacheConfig {
            db_path: PathBuf::from(":memory:"),
            limits: CacheLimits {
                max_chats: 10,
                max_messages_per_chat: 50,
                max_bytes: 0,
            },
            flush_debounce: Duration::from_millis(5),
        }
    }

    fn chat_summary(chat_id: i64, title: &str, last_message_at: i64) -> ChatSummary {
        ChatSummary {
            chat_id: ChatId(chat_id),
            title: title.to_string(),
            peer_kind: ChatPeerKind::User,
            last_message_id: Some(MessageId(last_message_at)),
            last_message_at: Some(last_message_at),
            unread_count: Some(1),
        }
    }

    fn message_new(chat_id: i64, message_id: i64, timestamp: i64, outgoing: bool) -> MessageNew {
        MessageNew {
            chat_id: ChatId(chat_id),
            message_id: MessageId(message_id),
            author_id: UserId(42),
            author_name: None,
            timestamp,
            text: format!("message-{}", message_id),
            outgoing,
        }
    }

    #[tokio::test]
    async fn selects_most_recent_chat_when_none_selected() {
        let store: Arc<dyn CacheStore> = Arc::new(InMemoryStore::default());
        let manager = CacheManager::spawn(store, cache_config())
            .await
            .expect("spawn cache manager");

        manager.upsert_chat(chat_summary(1, "General", 100));
        manager.upsert_chat(chat_summary(2, "Product", 300));

        let mut bridge = UiCacheBridge::new(None, 32);
        let selected = bridge.refresh(&manager);

        assert_eq!(selected, Some(ChatId(2)));
        assert_eq!(bridge.state.chats.len(), 2);
        assert_eq!(bridge.state.chats[0].id, 2);
        assert!(bridge.state.chats[0].is_selected);
        assert_eq!(bridge.state.chats[0].title, "Product");

        manager.shutdown().await;
    }

    #[tokio::test]
    async fn maps_messages_for_selected_chat() {
        let store: Arc<dyn CacheStore> = Arc::new(InMemoryStore::default());
        let manager = CacheManager::spawn(store, cache_config())
            .await
            .expect("spawn cache manager");

        manager.upsert_chat(chat_summary(1, "General", 100));
        manager.upsert_chat(chat_summary(2, "Product", 200));
        manager.apply_event(&DomainEvent::MessageNew(message_new(1, 1, 0, false)));
        manager.apply_event(&DomainEvent::MessageNew(message_new(2, 1, 60, true)));
        manager.apply_event(&DomainEvent::MessageNew(message_new(2, 2, 120, false)));

        let mut bridge = UiCacheBridge::new(None, 32);
        bridge.set_selected_chat(Some(ChatId(2)));
        bridge.refresh(&manager);

        assert_eq!(bridge.state.messages.len(), 2);
        assert_eq!(bridge.state.messages[0].id, 1);
        assert_eq!(bridge.state.messages[0].author, "You");
        assert_eq!(bridge.state.messages[0].timestamp, "00:01");
        assert_eq!(bridge.state.messages[1].id, 2);
        assert_eq!(bridge.state.messages[1].author, "User 42");
        assert_eq!(bridge.state.messages[1].timestamp, "00:02");

        manager.shutdown().await;
    }

    #[tokio::test]
    async fn maps_message_author_names_from_cache() {
        let store: Arc<dyn CacheStore> = Arc::new(InMemoryStore::default());
        let manager = CacheManager::spawn(store, cache_config())
            .await
            .expect("spawn cache manager");

        manager.upsert_chat(chat_summary(1, "General", 100));
        manager.apply_event(&DomainEvent::MessageNew(message_new(1, 1, 60, false)));
        manager.upsert_user(CachedUser {
            user_id: UserId(42),
            display_name: "Ada Lovelace".to_string(),
        });

        let mut bridge = UiCacheBridge::new(None, 32);
        bridge.set_selected_chat(Some(ChatId(1)));
        bridge.refresh(&manager);

        assert_eq!(bridge.state.messages.len(), 1);
        assert_eq!(bridge.state.messages[0].author, "Ada Lovelace");

        manager.shutdown().await;
    }

    #[test]
    fn sync_selected_chat_from_state_updates_selection() {
        let mut bridge = UiCacheBridge::new(None, 32);
        bridge.state.chats = vec![
            ChatListItem {
                id: 10,
                title: "General".to_string(),
                unread: 0,
                is_selected: false,
            },
            ChatListItem {
                id: 11,
                title: "Design".to_string(),
                unread: 0,
                is_selected: true,
            },
        ];

        let changed = bridge.sync_selected_chat_from_state();

        assert!(changed);
        assert_eq!(bridge.selected_chat(), Some(ChatId(11)));
    }

    #[test]
    fn sync_selected_chat_from_state_no_change() {
        let mut bridge = UiCacheBridge::new(None, 32);
        bridge.state.chats = vec![ChatListItem {
            id: 10,
            title: "General".to_string(),
            unread: 0,
            is_selected: true,
        }];
        bridge.set_selected_chat(Some(ChatId(10)));

        let changed = bridge.sync_selected_chat_from_state();

        assert!(!changed);
        assert_eq!(bridge.selected_chat(), Some(ChatId(10)));
    }
}
