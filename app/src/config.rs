use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use telegram_llm_core::telegram::SendPipelineConfig;
use thiserror::Error;
use tracing_subscriber::filter::LevelFilter;

use crate::prompt::AuthMethod;

const DEFAULT_SESSION_PATH: &str = "data/telegram.session";
const DEFAULT_UPDATE_BUFFER: usize = 1024;
const DEFAULT_AUTH_METHOD: AuthMethod = AuthMethod::Phone;
const DEFAULT_CONFIG_PATH: &str = "app/config/app.toml";
const DEFAULT_LOG_FILE_PATH: &str = "data/logs/app.log";
const DEFAULT_ERROR_LOG_PATH: &str = "data/logs/app-error.log";
const DEFAULT_SEND_QUEUE_LIMIT: usize = 256;
const DEFAULT_SEND_RETRY_BASE_DELAY_MS: u64 = 500;
const DEFAULT_SEND_RETRY_MAX_DELAY_MS: u64 = 30_000;
const DEFAULT_LOG_LEVEL: LevelFilter = LevelFilter::INFO;
const DEFAULT_LOG_FORMAT: LogFormat = LogFormat::Plain;
const DEFAULT_LOG_ROTATION: LogRotation = LogRotation::Size;
const DEFAULT_ROTATION_MAX_SIZE_MB: u64 = 1;
const DEFAULT_ROTATION_MAX_FILES: usize = 20;
const DEFAULT_LOG_CONTENT: bool = true;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub api_id: i32,
    pub api_hash: String,
    pub session_path: PathBuf,
    pub update_buffer: usize,
    pub send_queue_limit: usize,
    pub send_retry_max_attempts: Option<u32>,
    pub send_retry_base_delay_ms: u64,
    pub send_retry_max_delay_ms: u64,
    pub phone_number: Option<String>,
    pub auth_method: AuthMethod,
    pub log_file_path: PathBuf,
    pub error_log_path: PathBuf,
    pub log_level: LevelFilter,
    pub log_format: LogFormat,
    pub log_rotation: LogRotation,
    pub rotation_max_size_bytes: u64,
    pub rotation_max_files: usize,
    pub log_content: bool,
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
    #[error("invalid log format: {0}")]
    InvalidLogFormat(String),
    #[error("invalid log rotation: {0}")]
    InvalidLogRotation(String),
    #[error("invalid log rotation size: {0}")]
    InvalidLogRotationSize(String),
    #[error("invalid log rotation files: {0}")]
    InvalidLogRotationFiles(String),
    #[error("failed to read config file: {0}")]
    ConfigRead(String),
    #[error("failed to resolve current directory: {0}")]
    CurrentDir(String),
}

#[derive(Debug, Deserialize)]
struct FileConfig {
    auth: Option<AuthSection>,
    logging: Option<LoggingSection>,
    telegram: Option<TelegramSection>,
}

#[derive(Debug, Deserialize)]
struct AuthSection {
    default_method: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TelegramSection {
    update_buffer: Option<usize>,
    send_queue_limit: Option<usize>,
    send_retry_max_attempts: Option<u32>,
    send_retry_base_delay_ms: Option<u64>,
    send_retry_max_delay_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct LoggingSection {
    log_file: Option<String>,
    error_log_file: Option<String>,
    level: Option<String>,
    format: Option<String>,
    rotation: Option<String>,
    rotation_max_size_mb: Option<u64>,
    rotation_max_files: Option<usize>,
    log_content: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogRotation {
    Size,
    Daily,
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
            Err(_) => file_config
                .as_ref()
                .and_then(|config| config.telegram.as_ref())
                .and_then(|telegram| telegram.update_buffer)
                .unwrap_or(DEFAULT_UPDATE_BUFFER),
        };

        let send_queue_limit = file_config
            .as_ref()
            .and_then(|config| config.telegram.as_ref())
            .and_then(|telegram| telegram.send_queue_limit)
            .unwrap_or(DEFAULT_SEND_QUEUE_LIMIT);
        let send_queue_limit = normalize_send_queue_limit(send_queue_limit);

        let send_retry_max_attempts = file_config
            .as_ref()
            .and_then(|config| config.telegram.as_ref())
            .and_then(|telegram| telegram.send_retry_max_attempts)
            .and_then(normalize_send_retry_attempts);

        let send_retry_base_delay_ms = file_config
            .as_ref()
            .and_then(|config| config.telegram.as_ref())
            .and_then(|telegram| telegram.send_retry_base_delay_ms)
            .unwrap_or(DEFAULT_SEND_RETRY_BASE_DELAY_MS);

        let send_retry_max_delay_ms = file_config
            .as_ref()
            .and_then(|config| config.telegram.as_ref())
            .and_then(|telegram| telegram.send_retry_max_delay_ms)
            .unwrap_or(DEFAULT_SEND_RETRY_MAX_DELAY_MS);
        let send_retry_max_delay_ms = send_retry_max_delay_ms.max(send_retry_base_delay_ms);

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

        let log_file_path = file_config
            .as_ref()
            .and_then(|config| config.logging.as_ref())
            .and_then(|logging| logging.log_file.as_ref())
            .map(|raw| parse_log_path(raw.to_string()))
            .transpose()?;

        let log_file_path = match log_file_path {
            Some(path) => path,
            None => resolve_path(DEFAULT_LOG_FILE_PATH)?,
        };

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

        let log_format = file_config
            .as_ref()
            .and_then(|config| config.logging.as_ref())
            .and_then(|logging| logging.format.as_ref())
            .map(|raw| parse_log_format(raw.to_string()))
            .transpose()?
            .unwrap_or(DEFAULT_LOG_FORMAT);

        let log_rotation = file_config
            .as_ref()
            .and_then(|config| config.logging.as_ref())
            .and_then(|logging| logging.rotation.as_ref())
            .map(|raw| parse_log_rotation(raw.to_string()))
            .transpose()?
            .unwrap_or(DEFAULT_LOG_ROTATION);

        let rotation_max_size_mb = file_config
            .as_ref()
            .and_then(|config| config.logging.as_ref())
            .and_then(|logging| logging.rotation_max_size_mb)
            .unwrap_or(DEFAULT_ROTATION_MAX_SIZE_MB);

        let rotation_max_size_bytes = parse_rotation_size_mb(rotation_max_size_mb.to_string())?;

        let rotation_max_files = file_config
            .as_ref()
            .and_then(|config| config.logging.as_ref())
            .and_then(|logging| logging.rotation_max_files)
            .unwrap_or(DEFAULT_ROTATION_MAX_FILES);

        let rotation_max_files = parse_rotation_files(rotation_max_files.to_string())?;

        let log_content = file_config
            .as_ref()
            .and_then(|config| config.logging.as_ref())
            .and_then(|logging| logging.log_content)
            .unwrap_or(DEFAULT_LOG_CONTENT);

        Ok(Self {
            api_id,
            api_hash,
            session_path,
            update_buffer,
            send_queue_limit,
            send_retry_max_attempts,
            send_retry_base_delay_ms,
            send_retry_max_delay_ms,
            phone_number,
            auth_method,
            log_file_path,
            error_log_path,
            log_level,
            log_format,
            log_rotation,
            rotation_max_size_bytes,
            rotation_max_files,
            log_content,
        })
    }

    pub fn send_pipeline_config(&self) -> SendPipelineConfig {
        SendPipelineConfig {
            queue_limit: self.send_queue_limit,
            max_retry_attempts: self.send_retry_max_attempts,
            retry_base_delay: Duration::from_millis(self.send_retry_base_delay_ms),
            retry_max_delay: Duration::from_millis(self.send_retry_max_delay_ms),
        }
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

fn parse_log_format(raw: String) -> Result<LogFormat, ConfigError> {
    match raw.trim().to_lowercase().as_str() {
        "plain" => Ok(LogFormat::Plain),
        other => Err(ConfigError::InvalidLogFormat(other.to_string())),
    }
}

fn parse_log_rotation(raw: String) -> Result<LogRotation, ConfigError> {
    match raw.trim().to_lowercase().as_str() {
        "size" => Ok(LogRotation::Size),
        "daily" => Ok(LogRotation::Daily),
        other => Err(ConfigError::InvalidLogRotation(other.to_string())),
    }
}

fn normalize_send_queue_limit(value: usize) -> usize {
    if value == 0 {
        DEFAULT_SEND_QUEUE_LIMIT
    } else {
        value
    }
}

fn normalize_send_retry_attempts(value: u32) -> Option<u32> {
    if value == 0 {
        None
    } else {
        Some(value)
    }
}

fn parse_rotation_size_mb(raw: String) -> Result<u64, ConfigError> {
    let trimmed = raw.trim();
    let value = trimmed
        .parse::<u64>()
        .map_err(|_| ConfigError::InvalidLogRotationSize(raw.clone()))?;
    if value == 0 {
        return Err(ConfigError::InvalidLogRotationSize(raw));
    }
    Ok(value * 1024 * 1024)
}

fn parse_rotation_files(raw: String) -> Result<usize, ConfigError> {
    let trimmed = raw.trim();
    let value = trimmed
        .parse::<usize>()
        .map_err(|_| ConfigError::InvalidLogRotationFiles(raw.clone()))?;
    if value == 0 {
        return Err(ConfigError::InvalidLogRotationFiles(raw));
    }
    Ok(value)
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
        let temp_path = std::env::temp_dir().join("telegram-llm-tui-missing-update-config.toml");
        let _config = EnvGuard::set("APP_CONFIG_PATH", temp_path.to_string_lossy().as_ref());

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
    fn update_buffer_reads_from_config_file() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();

        let temp_path = std::env::temp_dir().join("telegram-llm-tui-update-config.toml");
        let _config = EnvGuard::set("APP_CONFIG_PATH", temp_path.to_string_lossy().as_ref());
        std::fs::write(&temp_path, "[telegram]\nupdate_buffer = 256\n").unwrap();

        let result = AppConfig::from_env();
        let _ = std::fs::remove_file(&temp_path);

        let config = result.unwrap();
        assert_eq!(config.update_buffer, 256);
    }

    #[test]
    fn update_buffer_env_overrides_config_file() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();

        let temp_path = std::env::temp_dir().join("telegram-llm-tui-update-config.toml");
        let _config = EnvGuard::set("APP_CONFIG_PATH", temp_path.to_string_lossy().as_ref());
        let _buffer = EnvGuard::set("TELEGRAM_UPDATE_BUFFER", "42");
        std::fs::write(&temp_path, "[telegram]\nupdate_buffer = 256\n").unwrap();

        let result = AppConfig::from_env();
        let _ = std::fs::remove_file(&temp_path);

        let config = result.unwrap();
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

    #[test]
    fn log_file_path_defaults_when_missing() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();
        let _config = EnvGuard::unset("APP_CONFIG_PATH");

        let config = AppConfig::from_env().unwrap();
        let path = config.log_file_path.to_string_lossy();
        assert!(path.ends_with(DEFAULT_LOG_FILE_PATH));
    }

    #[test]
    fn log_file_path_reads_from_config_file() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();

        let temp_path = std::env::temp_dir().join("telegram-llm-tui-log-file.toml");
        let _config = EnvGuard::set("APP_CONFIG_PATH", temp_path.to_string_lossy().as_ref());
        std::fs::write(&temp_path, "[logging]\nlog_file = \"logs/test.log\"\n").unwrap();

        let result = AppConfig::from_env();
        let _ = std::fs::remove_file(&temp_path);

        let config = result.unwrap();
        let path = config.log_file_path.to_string_lossy();
        assert!(path.ends_with("logs/test.log"));
    }

    #[test]
    fn log_format_reads_from_config_file() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();

        let temp_path = std::env::temp_dir().join("telegram-llm-tui-log-format.toml");
        let _config = EnvGuard::set("APP_CONFIG_PATH", temp_path.to_string_lossy().as_ref());
        std::fs::write(&temp_path, "[logging]\nformat = \"plain\"\n").unwrap();

        let result = AppConfig::from_env();
        let _ = std::fs::remove_file(&temp_path);

        let config = result.unwrap();
        assert_eq!(config.log_format, LogFormat::Plain);
    }

    #[test]
    fn log_rotation_reads_from_config_file() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();

        let temp_path = std::env::temp_dir().join("telegram-llm-tui-log-rotation.toml");
        let _config = EnvGuard::set("APP_CONFIG_PATH", temp_path.to_string_lossy().as_ref());
        std::fs::write(&temp_path, "[logging]\nrotation = \"daily\"\n").unwrap();

        let result = AppConfig::from_env();
        let _ = std::fs::remove_file(&temp_path);

        let config = result.unwrap();
        assert_eq!(config.log_rotation, LogRotation::Daily);
    }

    #[test]
    fn rotation_limits_read_from_config_file() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();

        let temp_path = std::env::temp_dir().join("telegram-llm-tui-log-rotation-limits.toml");
        let _config = EnvGuard::set("APP_CONFIG_PATH", temp_path.to_string_lossy().as_ref());
        std::fs::write(
            &temp_path,
            "[logging]\nrotation_max_size_mb = 2\nrotation_max_files = 5\n",
        )
        .unwrap();

        let result = AppConfig::from_env();
        let _ = std::fs::remove_file(&temp_path);

        let config = result.unwrap();
        assert_eq!(config.rotation_max_size_bytes, 2 * 1024 * 1024);
        assert_eq!(config.rotation_max_files, 5);
    }

    #[test]
    fn log_content_reads_from_config_file() {
        let _lock = env_lock().lock().unwrap();
        let (_id, _hash) = set_required_env();

        let temp_path = std::env::temp_dir().join("telegram-llm-tui-log-content.toml");
        let _config = EnvGuard::set("APP_CONFIG_PATH", temp_path.to_string_lossy().as_ref());
        std::fs::write(&temp_path, "[logging]\nlog_content = false\n").unwrap();

        let result = AppConfig::from_env();
        let _ = std::fs::remove_file(&temp_path);

        let config = result.unwrap();
        assert!(!config.log_content);
    }
}
