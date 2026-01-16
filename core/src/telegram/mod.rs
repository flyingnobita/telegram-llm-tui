pub mod auth;
pub mod bootstrap;
pub mod cache;
pub mod dialogs;
pub mod error;
pub mod events;
pub mod history;
pub mod send;
pub mod updates;

pub use auth::{AuthFlow, AuthResult, PhoneLogin, QrLogin, QrLoginResult};
pub use bootstrap::{
    EventDropPolicy, EventStreamConfig, TelegramBootstrap, TelegramConfig, UpdatesConfig,
};
pub use cache::{
    CacheConfig, CacheError, CacheLimits, CacheManager, CacheSnapshot, CacheStore, CachedMessage,
    CachedUser, ChatPeerKind, ChatSummary, SqliteCacheStore,
};
pub use dialogs::{fetch_dialog_summaries, fetch_dialogs, DialogSnapshot};
pub use error::{Result, TelegramError};
pub use events::{
    spawn_domain_event_pump, ChatId, DomainEvent, EventMapper, EventReceiver, EventStream,
    MessageEdited, MessageId, MessageNew, ReadReceipt, Typing, UserId,
};
pub use grammers_client::Client as TelegramClient;
pub use history::fetch_recent_messages;
pub use send::{
    spawn_grammers_send_pipeline, spawn_send_pipeline, SendEnqueueError, SendFailure, SendId,
    SendPipeline, SendPipelineConfig, SendRequest, SendResult, SendStatus, SendTicket,
};
pub use updates::{
    spawn_telegram_update_pump, spawn_update_pump, UpdateEvent, UpdatePump, UpdateSource,
};
