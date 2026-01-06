use thiserror::Error;

#[derive(Debug, Error)]
pub enum TelegramError {
    #[error("telegram invocation error: {0}")]
    Invocation(#[from] grammers_mtsender::InvocationError),
    #[error("sign in error: {0}")]
    SignIn(Box<grammers_client::SignInError>),
    #[error("sqlite session error: {0}")]
    Sqlite(#[from] sqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("update pump already started or stopped")]
    UpdatePumpUnavailable,
}

pub type Result<T> = std::result::Result<T, TelegramError>;

impl From<grammers_client::SignInError> for TelegramError {
    fn from(err: grammers_client::SignInError) -> Self {
        Self::SignIn(Box::new(err))
    }
}
