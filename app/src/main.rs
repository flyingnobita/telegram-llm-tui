mod config;
mod prompt;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use telegram_llm_core::telegram::{
    AuthResult, CacheManager, QrLoginResult, SqliteCacheStore, TelegramBootstrap, TelegramConfig,
};
use time::{format_description, UtcOffset};
use tokio::sync::broadcast::error::RecvError;
use tracing::{info, warn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;

use crate::config::{AppConfig, LogFormat, LogRotation};
use crate::prompt::{prompt_line, prompt_secret, AuthMethod};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(4 * 1024 * 1024)
        .build()?;
    runtime.block_on(async_main())
}

async fn async_main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let config = AppConfig::from_env()?;
    init_tracing(&config)?;
    info!("loaded configuration");

    let cache_store = Arc::new(SqliteCacheStore::new(config.cache_db_path.clone()));
    let cache_manager = CacheManager::spawn(cache_store, config.cache_config()).await?;

    let mut telegram_config = TelegramConfig::new(
        config.api_id,
        config.api_hash.clone(),
        config.session_path.clone(),
    );
    telegram_config.send_pipeline = config.send_pipeline_config();

    let mut bootstrap = TelegramBootstrap::connect(telegram_config).await?;
    let auth_flow = bootstrap.auth_flow();

    if !auth_flow.is_authorized().await? {
        info!("authentication required");
        let method = config.auth_method;
        info!(method = ?method, "using default auth method");
        match method {
            AuthMethod::Phone => {
                run_phone_login(&auth_flow, config.phone_number.as_deref()).await?
            }
            AuthMethod::Qr => run_qr_login(&auth_flow).await?,
        }
    } else {
        info!("already authorized");
    }

    info!("starting domain event stream");
    let event_stream = bootstrap.spawn_event_stream(config.update_buffer)?;
    let mut event_rx = event_stream.subscribe();

    tokio::select! {
        _ = async {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        cache_manager.apply_event(&event);
                        info!(?event, "received domain event");
                    }
                    Err(RecvError::Lagged(_)) => {
                        continue;
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        } => {}
        _ = tokio::signal::ctrl_c() => {
            info!("shutdown requested");
        }
    }

    event_stream.stop().await;
    cache_manager.shutdown().await;
    bootstrap.shutdown().await;
    info!("shutdown complete");
    Ok(())
}

fn init_tracing(config: &AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    ensure_parent_dir(&config.log_file_path)?;
    ensure_parent_dir(&config.error_log_path)?;

    let log_writer = build_log_writer(
        &config.log_file_path,
        config.log_rotation,
        config.rotation_max_size_bytes,
        config.rotation_max_files,
    )?;
    let error_writer = build_log_writer(
        &config.error_log_path,
        config.log_rotation,
        config.rotation_max_size_bytes,
        config.rotation_max_files,
    )?;

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level_filter_directive(config.log_level)));
    match config.log_format {
        LogFormat::Plain => {
            let stdout_timer = build_timer();
            let file_timer = build_timer();
            let error_timer = build_timer();
            let stdout_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_ansi(true)
                .with_timer(stdout_timer)
                .with_filter(filter.clone());
            let file_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_writer(log_writer)
                .with_ansi(false)
                .with_timer(file_timer)
                .with_filter(filter);
            let error_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_writer(error_writer)
                .with_ansi(false)
                .with_timer(error_timer)
                .with_filter(tracing_subscriber::filter::LevelFilter::ERROR);

            tracing_subscriber::registry()
                .with(stdout_layer)
                .with(file_layer)
                .with(error_layer)
                .init();
        }
    }
    Ok(())
}

fn level_filter_directive(level: tracing_subscriber::filter::LevelFilter) -> &'static str {
    match level {
        tracing_subscriber::filter::LevelFilter::ERROR => "error",
        tracing_subscriber::filter::LevelFilter::WARN => "warn",
        tracing_subscriber::filter::LevelFilter::INFO => "info",
        tracing_subscriber::filter::LevelFilter::DEBUG => "debug",
        tracing_subscriber::filter::LevelFilter::TRACE => "trace",
        tracing_subscriber::filter::LevelFilter::OFF => "off",
    }
}

fn build_timer() -> impl tracing_subscriber::fmt::time::FormatTime {
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let format = format_description::parse(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3][offset_hour sign:mandatory]:[offset_minute]",
    )
    .expect("valid time format");
    tracing_subscriber::fmt::time::OffsetTime::new(offset, format)
}

fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn build_log_writer(
    path: &Path,
    rotation: LogRotation,
    max_size_bytes: u64,
    max_files: usize,
) -> Result<SharedWriter, Box<dyn std::error::Error>> {
    let writer: Box<dyn Write + Send> = match rotation {
        LogRotation::Daily => {
            let parent = path.parent().unwrap_or_else(|| Path::new("."));
            let file_name = path
                .file_name()
                .ok_or_else(|| io::Error::other("missing log file name"))?;
            Box::new(tracing_appender::rolling::daily(parent, file_name))
        }
        LogRotation::Size => Box::new(RotatingFileWriter::new(
            path.to_path_buf(),
            max_size_bytes,
            max_files,
        )?),
    };
    Ok(SharedWriter::new(writer))
}

struct SharedWriter {
    inner: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl SharedWriter {
    fn new(writer: Box<dyn Write + Send>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(writer)),
        }
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedWriter {
    type Writer = SharedWriterGuard<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        SharedWriterGuard {
            guard: self.inner.lock().unwrap(),
        }
    }
}

struct SharedWriterGuard<'a> {
    guard: MutexGuard<'a, Box<dyn Write + Send>>,
}

impl Write for SharedWriterGuard<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.guard.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.guard.flush()
    }
}

struct RotatingFileWriter {
    base_path: PathBuf,
    max_bytes: u64,
    max_files: usize,
    file: std::fs::File,
    size: u64,
}

impl RotatingFileWriter {
    fn new(base_path: PathBuf, max_bytes: u64, max_files: usize) -> io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&base_path)?;
        let size = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        Ok(Self {
            base_path,
            max_bytes,
            max_files,
            file,
            size,
        })
    }

    fn rotate_if_needed(&mut self, incoming_len: usize) -> io::Result<()> {
        if self.max_bytes == 0 || self.max_files == 0 {
            return Ok(());
        }
        let incoming = incoming_len as u64;
        if self.size + incoming <= self.max_bytes {
            return Ok(());
        }
        self.rotate_files()?;
        self.file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.base_path)?;
        self.size = 0;
        Ok(())
    }

    fn rotate_files(&self) -> io::Result<()> {
        let base = self.base_path.to_string_lossy().to_string();
        let oldest = format!("{}.{}", base, self.max_files);
        let _ = std::fs::remove_file(&oldest);

        for idx in (1..=self.max_files).rev() {
            let src = if idx == 1 {
                base.clone()
            } else {
                format!("{}.{}", base, idx - 1)
            };
            let dst = format!("{}.{}", base, idx);
            if Path::new(&src).exists() {
                std::fs::rename(src, dst)?;
            }
        }
        Ok(())
    }
}

impl Write for RotatingFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.rotate_if_needed(buf.len())?;
        let written = self.file.write(buf)?;
        self.size = self.size.saturating_add(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

async fn run_phone_login(
    auth_flow: &telegram_llm_core::telegram::AuthFlow<
        telegram_llm_core::telegram::auth::GrammersAuthClient,
    >,
    default_phone: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let phone = match default_phone {
        Some(phone) => phone.to_string(),
        None => prompt_line("Phone number: ")?,
    };
    info!("requesting login code");
    let login = auth_flow.begin_phone_login(phone.trim()).await?;

    loop {
        let code = prompt_line("Login code: ")?;
        match auth_flow.submit_phone_code(&login, code.trim()).await? {
            AuthResult::Authorized => {
                info!("phone login authorized");
                break;
            }
            AuthResult::PasswordRequired(token) => {
                info!("2fa password required");
                let password = prompt_secret("2fa password: ")?;
                match auth_flow.submit_password(token, password.trim()).await? {
                    AuthResult::Authorized => {
                        info!("2fa authorized");
                        break;
                    }
                    AuthResult::InvalidPassword => {
                        warn!("invalid password, retry");
                    }
                    AuthResult::SignUpRequired => {
                        warn!("sign up required, use official client");
                        break;
                    }
                    AuthResult::InvalidCode | AuthResult::PasswordRequired(_) => {}
                }
            }
            AuthResult::InvalidCode => {
                warn!("invalid code, retry");
            }
            AuthResult::SignUpRequired => {
                warn!("sign up required, use official client");
                break;
            }
            AuthResult::InvalidPassword => {
                warn!("invalid password, retry");
            }
        }
    }

    Ok(())
}

async fn run_qr_login(
    auth_flow: &telegram_llm_core::telegram::AuthFlow<
        telegram_llm_core::telegram::auth::GrammersAuthClient,
    >,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("requesting qr login token");
    let mut pending = match auth_flow.begin_qr_login().await? {
        QrLoginResult::Authorized => {
            info!("qr login already authorized");
            return Ok(());
        }
        QrLoginResult::Pending(login) => login,
    };

    loop {
        let url = format!(
            "tg://login?token={}",
            URL_SAFE_NO_PAD.encode(&pending.token)
        );
        println!("Scan QR code from this URL: {url}");
        info!("waiting for qr approval");

        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            match auth_flow.poll_qr_login(&pending).await? {
                QrLoginResult::Authorized => {
                    info!("qr login authorized");
                    return Ok(());
                }
                QrLoginResult::Pending(login) => {
                    if login.token != pending.token || login.dc_id != pending.dc_id {
                        pending = login;
                        break;
                    }
                }
            }
        }
    }
}
