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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageNew {
    pub chat_id: ChatId,
    pub message_id: MessageId,
    pub author_id: UserId,
    pub timestamp: i64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageEdited {
    pub chat_id: ChatId,
    pub message_id: MessageId,
    pub editor_id: UserId,
    pub timestamp: i64,
    pub text: String,
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
        let state_timestamp = update.state().date as i64;
        match update.raw() {
            tl::enums::Update::NewMessage(update) => self.map_message_new(&update.message),
            tl::enums::Update::NewChannelMessage(update) => self.map_message_new(&update.message),
            tl::enums::Update::EditMessage(update) => self.map_message_edited(&update.message),
            tl::enums::Update::EditChannelMessage(update) => {
                self.map_message_edited(&update.message)
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

    fn map_message_new(&self, message: &tl::enums::Message) -> Option<DomainEvent> {
        let fields = self.parse_message(message)?;
        Some(DomainEvent::MessageNew(MessageNew {
            chat_id: fields.chat_id,
            message_id: fields.message_id,
            author_id: fields.author_id,
            timestamp: fields.date,
            text: fields.text,
        }))
    }

    fn map_message_edited(&self, message: &tl::enums::Message) -> Option<DomainEvent> {
        let fields = self.parse_message(message)?;
        let timestamp = fields.edit_date.unwrap_or(fields.date);
        Some(DomainEvent::MessageEdited(MessageEdited {
            chat_id: fields.chat_id,
            message_id: fields.message_id,
            editor_id: fields.author_id,
            timestamp,
            text: fields.text,
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

    fn parse_message(&self, message: &tl::enums::Message) -> Option<ParsedMessage> {
        match message {
            tl::enums::Message::Message(message) => {
                let chat_id = ChatId(PeerId::from(message.peer_id.clone()).bot_api_dialog_id());
                let author_peer = message.from_id.as_ref().or(if message.out {
                    None
                } else {
                    Some(&message.peer_id)
                });
                let author_id = match author_peer.and_then(user_id_from_peer) {
                    Some(author_id) => author_id,
                    None => {
                        warn!(peer = ?message.peer_id, "message missing author user id");
                        return None;
                    }
                };
                Some(ParsedMessage {
                    chat_id,
                    message_id: MessageId(message.id as i64),
                    author_id,
                    date: message.date as i64,
                    edit_date: message.edit_date.map(|value| value as i64),
                    text: message.message.clone(),
                })
            }
            _ => {
                warn!(message = ?message, "unsupported message variant");
                None
            }
        }
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

struct ParsedMessage {
    chat_id: ChatId,
    message_id: MessageId,
    author_id: UserId,
    date: i64,
    edit_date: Option<i64>,
    text: String,
}

fn user_id_from_peer(peer: &tl::enums::Peer) -> Option<UserId> {
    match peer {
        tl::enums::Peer::User(user) => Some(UserId(user.user_id)),
        tl::enums::Peer::Chat(_) | tl::enums::Peer::Channel(_) => None,
    }
}
