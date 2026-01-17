use grammers_client::types::{Message as GrammersMessage, Peer, User};
use grammers_session::defs::PeerId;
use grammers_tl_types as tl;
use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use tracing::warn;

use crate::telegram::error::{Result, TelegramError};
use crate::telegram::updates::{UpdateEvent, UpdatePump};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChatId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthorId {
    User(UserId),
    Chat(ChatId),
}

impl AuthorId {
    pub fn as_user(self) -> Option<UserId> {
        match self {
            AuthorId::User(user_id) => Some(user_id),
            AuthorId::Chat(_) => None,
        }
    }

    pub fn as_chat(self) -> Option<ChatId> {
        match self {
            AuthorId::User(_) => None,
            AuthorId::Chat(chat_id) => Some(chat_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageNew {
    pub chat_id: ChatId,
    pub message_id: MessageId,
    pub author_id: AuthorId,
    pub author_name: Option<String>,
    pub timestamp: i64,
    pub text: String,
    pub outgoing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageEdited {
    pub chat_id: ChatId,
    pub message_id: MessageId,
    pub editor_id: AuthorId,
    pub editor_name: Option<String>,
    pub timestamp: i64,
    pub text: String,
    pub outgoing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadReceipt {
    pub chat_id: ChatId,
    pub reader_id: UserId,
    pub timestamp: i64,
    pub last_read_message_id: MessageId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typing {
    pub chat_id: ChatId,
    pub user_id: UserId,
    pub timestamp: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainEvent {
    MessageNew(MessageNew),
    MessageEdited(MessageEdited),
    ReadReceipt(ReadReceipt),
    Typing(Typing),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EventMapper;

impl EventMapper {
    pub fn new() -> Self {
        Self
    }

    pub fn map_update(&self, update: &grammers_client::Update) -> Option<DomainEvent> {
        match update {
            grammers_client::Update::NewMessage(message) => self.map_message_new(message),
            grammers_client::Update::MessageEdited(message) => self.map_message_edited(message),
            _ => self.map_raw_update(update),
        }
    }

    fn map_raw_update(&self, update: &grammers_client::Update) -> Option<DomainEvent> {
        let state_timestamp = update.state().date as i64;
        match update.raw() {
            tl::enums::Update::NewMessage(update) => self.map_raw_message_new(&update.message),
            tl::enums::Update::NewChannelMessage(update) => {
                self.map_raw_message_new(&update.message)
            }
            tl::enums::Update::EditMessage(update) => self.map_raw_message_edited(&update.message),
            tl::enums::Update::EditChannelMessage(update) => {
                self.map_raw_message_edited(&update.message)
            }
            tl::enums::Update::ReadHistoryOutbox(update) => {
                self.map_read_receipt(&update.peer, update.max_id, state_timestamp)
            }
            tl::enums::Update::UserTyping(update) => {
                self.map_typing_user(update.user_id, state_timestamp)
            }
            unsupported => {
                warn!(update = ?unsupported, "unsupported telegram update");
                None
            }
        }
    }

    fn map_message_new(&self, message: &GrammersMessage) -> Option<DomainEvent> {
        let fields = parse_message(&message.raw)?;
        let author_name = resolve_sender_display_name(message.sender()).or(fields.author_name);
        Some(DomainEvent::MessageNew(MessageNew {
            chat_id: fields.chat_id,
            message_id: fields.message_id,
            author_id: fields.author_id,
            author_name,
            timestamp: fields.date,
            text: fields.text,
            outgoing: fields.outgoing,
        }))
    }

    fn map_message_edited(&self, message: &GrammersMessage) -> Option<DomainEvent> {
        let fields = parse_message(&message.raw)?;
        let editor_name = resolve_sender_display_name(message.sender()).or(fields.author_name);
        let timestamp = fields.edit_date.unwrap_or(fields.date);
        Some(DomainEvent::MessageEdited(MessageEdited {
            chat_id: fields.chat_id,
            message_id: fields.message_id,
            editor_id: fields.author_id,
            editor_name,
            timestamp,
            text: fields.text,
            outgoing: fields.outgoing,
        }))
    }

    fn map_raw_message_new(&self, message: &tl::enums::Message) -> Option<DomainEvent> {
        let fields = parse_message(message)?;
        Some(DomainEvent::MessageNew(MessageNew {
            chat_id: fields.chat_id,
            message_id: fields.message_id,
            author_id: fields.author_id,
            author_name: fields.author_name,
            timestamp: fields.date,
            text: fields.text,
            outgoing: fields.outgoing,
        }))
    }

    fn map_raw_message_edited(&self, message: &tl::enums::Message) -> Option<DomainEvent> {
        let fields = parse_message(message)?;
        let timestamp = fields.edit_date.unwrap_or(fields.date);
        Some(DomainEvent::MessageEdited(MessageEdited {
            chat_id: fields.chat_id,
            message_id: fields.message_id,
            editor_id: fields.author_id,
            editor_name: fields.author_name,
            timestamp,
            text: fields.text,
            outgoing: fields.outgoing,
        }))
    }

    fn map_read_receipt(
        &self,
        peer: &tl::enums::Peer,
        max_id: i32,
        timestamp: i64,
    ) -> Option<DomainEvent> {
        let chat_id = ChatId(PeerId::from(peer.clone()).bot_api_dialog_id());
        let reader_id = match user_id_from_peer(peer) {
            Some(user_id) => user_id,
            None => {
                warn!(peer = ?peer, "read receipt missing user reader id");
                return None;
            }
        };
        Some(DomainEvent::ReadReceipt(ReadReceipt {
            chat_id,
            reader_id,
            timestamp,
            last_read_message_id: MessageId(max_id as i64),
        }))
    }

    fn map_typing_user(&self, user_id: i64, timestamp: i64) -> Option<DomainEvent> {
        let peer_id = PeerId::user(user_id);
        Some(DomainEvent::Typing(Typing {
            chat_id: ChatId(peer_id.bot_api_dialog_id()),
            user_id: UserId(user_id),
            timestamp,
        }))
    }
}

pub struct EventStream {
    sender: broadcast::Sender<DomainEvent>,
    stop_tx: watch::Sender<bool>,
    join: JoinHandle<()>,
    update_pump: Option<UpdatePump<grammers_client::Update, grammers_mtsender::InvocationError>>,
}

impl EventStream {
    pub fn subscribe(&self) -> EventReceiver {
        EventReceiver::from_receiver(self.sender.subscribe())
    }

    pub async fn stop(mut self) {
        let _ = self.stop_tx.send(true);
        let _ = self.join.await;
        if let Some(pump) = self.update_pump.take() {
            pump.stop().await;
        }
    }
}

pub struct EventReceiver {
    inner: broadcast::Receiver<DomainEvent>,
}

impl EventReceiver {
    pub fn from_receiver(receiver: broadcast::Receiver<DomainEvent>) -> Self {
        Self { inner: receiver }
    }

    pub async fn recv(&mut self) -> std::result::Result<DomainEvent, broadcast::error::RecvError> {
        match self.inner.recv().await {
            Ok(event) => Ok(event),
            Err(broadcast::error::RecvError::Lagged(count)) => {
                warn!(lagged = count, "event receiver lagged");
                Err(broadcast::error::RecvError::Lagged(count))
            }
            Err(err) => Err(err),
        }
    }
}

pub fn spawn_domain_event_pump(
    mut update_pump: UpdatePump<grammers_client::Update, grammers_mtsender::InvocationError>,
    buffer: usize,
) -> Result<EventStream> {
    let mut update_rx = update_pump
        .take_receiver()
        .ok_or(TelegramError::UpdatePumpUnavailable)?;
    let (sender, _) = broadcast::channel(buffer);
    let sender_task = sender.clone();
    let (stop_tx, mut stop_rx) = watch::channel(false);
    let mapper = EventMapper::new();

    let join = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = stop_rx.changed() => {
                    break;
                }
                update = update_rx.recv() => {
                    let Some(update) = update else {
                        break;
                    };
                    match update {
                        UpdateEvent::Update(update) => {
                            if let Some(event) = mapper.map_update(&update) {
                                if sender_task.send(event).is_err() {
                                    warn!("dropped domain event because no subscribers are active");
                                }
                            }
                        }
                        UpdateEvent::Error(err) => {
                            warn!(error = %err, "update pump error while mapping domain events");
                            break;
                        }
                    }
                }
            }
        }
    });

    Ok(EventStream {
        sender,
        stop_tx,
        join,
        update_pump: Some(update_pump),
    })
}

pub(crate) struct ParsedMessage {
    pub(crate) chat_id: ChatId,
    pub(crate) message_id: MessageId,
    pub(crate) author_id: AuthorId,
    pub(crate) author_name: Option<String>,
    pub(crate) date: i64,
    pub(crate) edit_date: Option<i64>,
    pub(crate) text: String,
    pub(crate) outgoing: bool,
}

pub(crate) fn parse_message(message: &tl::enums::Message) -> Option<ParsedMessage> {
    match message {
        tl::enums::Message::Message(message) => {
            let chat_id = ChatId(PeerId::from(message.peer_id.clone()).bot_api_dialog_id());
            let author_id =
                resolve_author_id(message.from_id.as_ref(), &message.peer_id, message.out);
            Some(ParsedMessage {
                chat_id,
                message_id: MessageId(message.id as i64),
                author_id,
                author_name: message
                    .post_author
                    .clone()
                    .filter(|name| !name.trim().is_empty()),
                date: message.date as i64,
                edit_date: message.edit_date.map(|value| value as i64),
                text: message.message.clone(),
                outgoing: message.out,
            })
        }
        tl::enums::Message::Service(message) => {
            let chat_id = ChatId(PeerId::from(message.peer_id.clone()).bot_api_dialog_id());
            let author_id =
                resolve_author_id(message.from_id.as_ref(), &message.peer_id, message.out);
            Some(ParsedMessage {
                chat_id,
                message_id: MessageId(message.id as i64),
                author_id,
                author_name: None,
                date: message.date as i64,
                edit_date: None,
                text: service_action_text(&message.action),
                outgoing: message.out,
            })
        }
        _ => None,
    }
}

fn service_action_text(action: &tl::enums::MessageAction) -> String {
    let detail = match action {
        tl::enums::MessageAction::ChatAddUser(action) => {
            if action.users.len() == 1 {
                "User joined".to_string()
            } else {
                format!("{} users joined", action.users.len())
            }
        }
        tl::enums::MessageAction::ChatDeleteUser(_) => "User left".to_string(),
        tl::enums::MessageAction::ChatJoinedByLink(_) => "Joined by link".to_string(),
        tl::enums::MessageAction::ChatJoinedByRequest => "Joined by request".to_string(),
        tl::enums::MessageAction::ChatCreate(action) => {
            format!("Chat created: {}", action.title)
        }
        tl::enums::MessageAction::ChatEditTitle(action) => {
            format!("Chat title changed to \"{}\"", action.title)
        }
        tl::enums::MessageAction::ChatEditPhoto(_) => "Chat photo updated".to_string(),
        tl::enums::MessageAction::ChatDeletePhoto => "Chat photo removed".to_string(),
        tl::enums::MessageAction::ChannelCreate(_) => "Channel created".to_string(),
        tl::enums::MessageAction::PinMessage => "Message pinned".to_string(),
        tl::enums::MessageAction::HistoryClear => "History cleared".to_string(),
        tl::enums::MessageAction::ContactSignUp => "Contact joined Telegram".to_string(),
        tl::enums::MessageAction::CustomAction(action) => action.message.clone(),
        _ => format!("{action:?}"),
    };

    format!("[Service] {detail}")
}

fn user_id_from_peer(peer: &tl::enums::Peer) -> Option<UserId> {
    match peer {
        tl::enums::Peer::User(user) => Some(UserId(user.user_id)),
        tl::enums::Peer::Chat(_) | tl::enums::Peer::Channel(_) => None,
    }
}

fn author_id_from_peer(peer: &tl::enums::Peer) -> AuthorId {
    match peer {
        tl::enums::Peer::User(user) => AuthorId::User(UserId(user.user_id)),
        tl::enums::Peer::Chat(_) | tl::enums::Peer::Channel(_) => {
            AuthorId::Chat(ChatId(PeerId::from(peer.clone()).bot_api_dialog_id()))
        }
    }
}

fn resolve_author_id(
    from_id: Option<&tl::enums::Peer>,
    peer_id: &tl::enums::Peer,
    outgoing: bool,
) -> AuthorId {
    match from_id {
        Some(peer) => author_id_from_peer(peer),
        None if outgoing => AuthorId::User(UserId(PeerId::self_user().bot_api_dialog_id())),
        None => author_id_from_peer(peer_id),
    }
}

fn resolve_sender_display_name(sender: Option<&Peer>) -> Option<String> {
    let sender = sender?;
    match sender {
        Peer::User(user) => resolve_user_display_name(user),
        Peer::Group(_) | Peer::Channel(_) => None,
    }
}

fn resolve_user_display_name(user: &User) -> Option<String> {
    let full_name = user.full_name();
    if !full_name.trim().is_empty() {
        return Some(full_name);
    }
    let username = user.username().map(str::trim).unwrap_or_default();
    if username.is_empty() {
        return None;
    }
    Some(format!("@{}", username.trim_start_matches('@')))
}
