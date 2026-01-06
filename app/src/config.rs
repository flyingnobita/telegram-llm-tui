use std::path::PathBuf;

use serde::Deserialize;
use thiserror::Error;
use tracing_subscriber::filter::LevelFilter;

use crate::prompt::AuthMethod;

const DEFAULT_SESSION_PATH: &str = "data/telegram.session";
const DEFAULT_UPDATE_BUFFER: usize = 100;
const DEFAULT_AUTH_METHOD: AuthMethod = AuthMethod::Phone;
const DEFAULT_CONFIG_PATH: &str = "app/config/app.toml";
const DEFAULT_ERROR_LOG_PATH: &str = "data/logs/app-error.log";
const DEFAULT_LOG_LEVEL: LevelFilter = LevelFilter::INFO;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub api_id: i32,
    pub api_hash: String,
    pub session_path: PathBuf,
    pub update_buffer: usize,
    pub phone_number: Option<String>,
    pub auth_method: AuthMethod,
    pub error_log_path: PathBuf,
    pub log_level: LevelFilter,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("missing environment variable: {0}")]
    Missing(&'static str),
    #[error("invalid api id: {0}")]
    InvalidApiId(String),
    #[error("invalid update buffer: {0}")]
    InvalidUpdateBuffer(String),
    #[error("invalid auth method: {0}")]
    InvalidAuthMethod(String),
    #[error("invalid log file path: {0}")]
    InvalidLogPath(String),
    #[error("invalid log level: {0}")]
    InvalidLogLevel(String),
    #[error("failed to read config file: {0}")]
    ConfigRead(String),
    #[error("failed to resolve current directory: {0}")]
    CurrentDir(String),
}

#[derive(Debug, Deserialize)]
struct FileConfig {
    auth: Option<AuthSection>,
    logging: Option<LoggingSection>,
}

#[derive(Debug, Deserialize)]
struct AuthSection {
    default_method: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LoggingSection {
    error_log_file: Option<String>,
    level: Option<String>,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let file_config = load_file_config()?;
        let api_id_raw = std::env::var("TELEGRAM_API_ID")
            .map_err(|_| ConfigError::Missing("TELEGRAM_API_ID"))?;
        let api_id = api_id_raw
            .parse::<i32>()
            .map_err(|_| ConfigError::InvalidApiId(api_id_raw))?;

        let api_hash = std::env::var("TELEGRAM_API_HASH")
            .map_err(|_| ConfigError::Missing("TELEGRAM_API_HASH"))?;

        let session_path = match std::env::var("TELEGRAM_SESSION_PATH") {
            Ok(path) => PathBuf::from(path),
            Err(_) => {
                let base = std::env::current_dir()
                    .map_err(|err| ConfigError::CurrentDir(err.to_string()))?;
                base.join(DEFAULT_SESSION_PATH)
            }
        };

        let update_buffer = match std::env::var("TELEGRAM_UPDATE_BUFFER") {
            Ok(raw) => raw
                .parse::<usize>()
                .map_err(|_| ConfigError::InvalidUpdateBuffer(raw))?,
            Err(_) => DEFAULT_UPDATE_BUFFER,
        };

        let phone_number = std::env::var("TELEGRAM_PHONE_NUMBER")
            .ok()
            .or_else(|| std::env::var("PHONE_NUMBER").ok())
            .and_then(|value| {
                let trimmed = value.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            });

        let auth_method = file_config
            .as_ref()
            .and_then(|config| config.auth.as_ref())
            .and_then(|auth| auth.default_method.as_ref())
            .map(|raw| parse_auth_method(raw.to_string()))
            .transpose()?
            .unwrap_or(DEFAULT_AUTH_METHOD);

        let error_log_path = file_config
            .as_ref()
            .and_then(|config| config.logging.as_ref())
            .and_then(|logging| logging.error_log_file.as_ref())
            .map(|raw| parse_log_path(raw.to_string()))
            .transpose()?;

        let error_log_path = match error_log_path {
            Some(path) => path,
            None => resolve_path(DEFAULT_ERROR_LOG_PATH)?,
        };

        let log_level = file_config
            .as_ref()
            .and_then(|config| config.logging.as_ref())
            .and_then(|logging| logging.level.as_ref())
            .map(|raw| parse_log_level(raw.to_string()))
            .transpose()?
            .unwrap_or(DEFAULT_LOG_LEVEL);

        Ok(Self {
            api_id,
            api_hash,
            session_path,
            update_buffer,
            phone_number,
            auth_method,
            error_log_path,
            log_level,
        })
    }
}

fn parse_auth_method(raw: String) -> Result<AuthMethod, ConfigError> {
    match raw.trim().to_lowercase().as_str() {
        "phone" => Ok(AuthMethod::Phone),
        "qr" => Ok(AuthMethod::Qr),
        other => Err(ConfigError::InvalidAuthMethod(other.to_string())),
    }
}

fn load_file_config() -> Result<Option<FileConfig>, ConfigError> {
    let path = std::env::var("APP_CONFIG_PATH").unwrap_or_else(|_| DEFAULT_CONFIG_PATH.to_string());
    let resolved = resolve_path(&path)?;
    if !resolved.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(&resolved)
        .map_err(|err| ConfigError::ConfigRead(err.to_string()))?;
    let config: FileConfig =
        toml::from_str(&contents).map_err(|err| ConfigError::ConfigRead(err.to_string()))?;
    Ok(Some(config))
}

fn resolve_path(raw: &str) -> Result<PathBuf, ConfigError> {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        return Ok(path);
    }
    let base = std::env::current_dir().map_err(|err| ConfigError::CurrentDir(err.to_string()))?;
    Ok(base.join(path))
}

fn parse_log_path(raw: String) -> Result<PathBuf, ConfigError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::InvalidLogPath(raw));
    }
    resolve_path(trimmed)
}

fn parse_log_level(raw: String) -> Result<LevelFilter, ConfigError> {
    raw.trim()
        .parse::<LevelFilter>()
        .map_err(|_| ConfigError::InvalidLogLevel(raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        key: &'static str,
        original: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let original = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, original }
        }

        fn unset(key: &'static str) -> Self {
            let original = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, original }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.original {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn set_required_env() -> (EnvGuard, EnvGuard) {
        (
            EnvGuard::set("TELEGRAM_API_ID", "123"),
            EnvGuard::set("TELEGRAM_API_HASH", "hash"),
        )
    }

    #[test]
    fn missing_api_id_returns_error() {
        let _lock = env_lock().lock().unwrap();
        let _hash = EnvGuard::set("TELEGRAM_API_HASH", "hash");
        let _id = EnvGuard::unset("TELEGRAM_API_ID");

        let err = AppConfig::from_env().unwrap_err();
        assert_eq!(err, ConfigError::Missing("TELEGRAM_API_ID"));
    }

    #[test]
    fn missing_api_hash_returns_error() {
        let _lock = env_lock().lock().unwrap();
        let _id = EnvGuard::set("TELEGRAM_API_ID", "123");
        let _hash = EnvGuard::unset("TELEGRAM_API_HASH");

        let err = AppConfig::from_env().unwrap_err();
        assert_eq!(err, ConfigError::Missing("TELEGRAM_API_HASH"));
    }

    #[test]
    fn invalid_api_id_returns_error() {
        let _lock = env_lock().lock().unwrap();
        let _hash = EnvGuard::set("TELEGRAM_API_HASH", "hash");
        let _id = EnvGuard::set("TELEGRAM_API_ID", "nope");

        let err = AppConfig::from_env().unwrap_err();
        assert_eq!(err, ConfigError::InvalidApiId("nope".to_string()));
    }

    #[test]
    fn default_session_path_is_used_when_missing() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();
        let _session = EnvGuard::unset("TELEGRAM_SESSION_PATH");

        let config = AppConfig::from_env().unwrap();
        let path = config.session_path.to_string_lossy();
        assert!(path.ends_with(DEFAULT_SESSION_PATH));
    }

    #[test]
    fn session_path_can_be_overridden() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();
        let _session = EnvGuard::set("TELEGRAM_SESSION_PATH", "/tmp/tg.session");

        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.session_path, PathBuf::from("/tmp/tg.session"));
    }

    #[test]
    fn update_buffer_defaults_when_missing() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();
        let _buffer = EnvGuard::unset("TELEGRAM_UPDATE_BUFFER");

        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.update_buffer, DEFAULT_UPDATE_BUFFER);
    }

    #[test]
    fn update_buffer_parses_from_env() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();
        let _buffer = EnvGuard::set("TELEGRAM_UPDATE_BUFFER", "42");

        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.update_buffer, 42);
    }

    #[test]
    fn phone_number_reads_from_env() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();
        let _phone = EnvGuard::set("TELEGRAM_PHONE_NUMBER", "+123");
        let _legacy = EnvGuard::unset("PHONE_NUMBER");

        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.phone_number, Some("+123".to_string()));
    }

    #[test]
    fn phone_number_falls_back_to_legacy_env() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();
        let _phone = EnvGuard::unset("TELEGRAM_PHONE_NUMBER");
        let _legacy = EnvGuard::set("PHONE_NUMBER", "+456");

        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.phone_number, Some("+456".to_string()));
    }

    #[test]
    fn auth_method_defaults_to_phone_when_config_missing() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();
        let temp_path = std::env::temp_dir().join("telegram-llm-tui-missing-config.toml");
        let _config = EnvGuard::set("APP_CONFIG_PATH", temp_path.to_string_lossy().as_ref());

        let config = AppConfig::from_env().unwrap();
        assert_eq!(config.auth_method, AuthMethod::Phone);
    }

    #[test]
    fn auth_method_reads_from_config_file() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();

        let temp_path = std::env::temp_dir().join("telegram-llm-tui-app-config.toml");
        let _config = EnvGuard::set("APP_CONFIG_PATH", temp_path.to_string_lossy().as_ref());
        std::fs::write(&temp_path, "[auth]\ndefault_method = \"qr\"\n").unwrap();

        let result = AppConfig::from_env();
        let _ = std::fs::remove_file(&temp_path);

        let config = result.unwrap();
        assert_eq!(config.auth_method, AuthMethod::Qr);
    }

    #[test]
    fn error_log_path_reads_from_config_file() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();

        let temp_path = std::env::temp_dir().join("telegram-llm-tui-log-config.toml");
        let _config = EnvGuard::set("APP_CONFIG_PATH", temp_path.to_string_lossy().as_ref());
        std::fs::write(
            &temp_path,
            "[logging]\nerror_log_file = \"data/logs/test-error.log\"\n",
        )
        .unwrap();

        let result = AppConfig::from_env();
        let _ = std::fs::remove_file(&temp_path);

        let config = result.unwrap();
        let path = config.error_log_path.to_string_lossy();
        assert!(path.ends_with("data/logs/test-error.log"));
    }

    #[test]
    fn log_level_reads_from_config_file() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();

        let temp_path = std::env::temp_dir().join("telegram-llm-tui-level-config.toml");
        let _config = EnvGuard::set("APP_CONFIG_PATH", temp_path.to_string_lossy().as_ref());
        std::fs::write(&temp_path, "[logging]\nlevel = \"debug\"\n").unwrap();

        let result = AppConfig::from_env();
        let _ = std::fs::remove_file(&temp_path);

        let config = result.unwrap();
        assert_eq!(config.log_level, LevelFilter::DEBUG);
    }
}
