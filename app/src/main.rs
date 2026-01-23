mod command;
mod config;
mod llm_workflow;
mod prompt;
mod tui;
mod ui_state;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use telegram_llm_core::telegram::{
    fetch_dialogs, fetch_recent_messages, AuthResult, CacheManager, CachedUser, ChatPeerKind,
    DomainEvent, QrLoginResult, SqliteCacheStore, TelegramBootstrap, TelegramClient,
    TelegramConfig, UserId,
};
use time::{format_description, UtcOffset};
use tokio::sync::mpsc;
use tracing::{info, warn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;

use crate::config::{AppConfig, LogFormat, LogRotation};
use crate::prompt::{prompt_line, prompt_secret, AuthMethod};
use crate::tui::run_tui_loop;
use crate::ui_state::UiCacheBridge;
use llm::openai::OpenAiProvider;
use llm::{LlmProvider, MockProvider};

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
    let console_gate = init_tracing(&config)?;
    info!("loaded configuration");
    info!(
        keymap = ?config.keymap_style,
        chat_list_width = config.chat_list_width,
        "configured tui layout"
    );

    let cache_store = Arc::new(SqliteCacheStore::new(config.cache_db_path.clone()));
    let cache_manager = Arc::new(CacheManager::spawn(cache_store, config.cache_config()).await?);
    let mut ui_bridge = UiCacheBridge::new(None, config.chat_list_width);
    ui_bridge.refresh(cache_manager.as_ref());

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

    let (cache_refresh_tx, cache_refresh_rx) = mpsc::unbounded_channel();
    let dialogs = match fetch_dialogs(bootstrap.client()).await {
        Ok(dialogs) => {
            let count = dialogs.len();
            for dialog in &dialogs {
                let summary = &dialog.summary;
                if summary.peer_kind == ChatPeerKind::User {
                    let fallback = format!("Chat {}", summary.chat_id.0);
                    if summary.title.trim() != fallback {
                        cache_manager.upsert_user(CachedUser {
                            user_id: UserId(summary.chat_id.0),
                            display_name: summary.title.clone(),
                        });
                    }
                }
                cache_manager.upsert_chat(summary.clone());
            }
            info!(count, "synced dialog summaries");
            if count > 0 {
                ui_bridge.refresh(cache_manager.as_ref());
            }
            ui_bridge.register_dialog_peers(&dialogs);
            Some(dialogs)
        }
        Err(err) => {
            warn!(error = %err, "failed to sync dialog summaries");
            None
        }
    };

    let send_pipeline = bootstrap.spawn_send_pipeline();

    info!("starting domain event stream");
    let event_stream = bootstrap.spawn_event_stream(config.update_buffer)?;
    let event_rx = event_stream.subscribe();

    let history_limit =
        effective_history_limit(config.history_per_chat, config.cache_max_messages_per_chat);
    let history_handle = if let Some(dialogs) = dialogs {
        if history_limit == 0 {
            None
        } else {
            let client = bootstrap.client().clone();
            let cache_manager = Arc::clone(&cache_manager);
            let cache_refresh_tx = cache_refresh_tx.clone();
            Some(tokio::spawn(async move {
                match sync_dialogs_and_history(
                    &client,
                    cache_manager.as_ref(),
                    dialogs,
                    history_limit,
                    cache_refresh_tx,
                )
                .await
                {
                    Ok(stats) => {
                        info!(
                            dialogs = stats.dialog_count,
                            messages = stats.message_count,
                            history_limit,
                            "synced dialog history"
                        );
                    }
                    Err(err) => {
                        warn!(error = %err, "failed to sync dialog history");
                    }
                }
            }))
        }
    } else {
        None
    };

    let llm_provider: Arc<dyn LlmProvider> = if config.llm.enabled {
        match config.llm.provider.as_str() {
            "lm_studio" => {
                let base_url = config.llm.lm_studio.base_url.clone();
                let model = config.llm.lm_studio.model.clone();
                match OpenAiProvider::new(base_url, model) {
                    Ok(provider) => Arc::new(provider),
                    Err(err) => {
                        warn!(%err, "failed to initialize lm_studio provider, falling back to mock");
                        Arc::new(MockProvider)
                    }
                }
            }
            "openai" => {
                // TODO: Implement OpenAI specific config (api_key, etc)
                warn!("openai provider not configured or implemented yet");
                Arc::new(MockProvider)
            }
            _ => Arc::new(MockProvider),
        }
    } else {
        Arc::new(MockProvider)
    };

    run_tui_loop(
        cache_manager.as_ref(),
        &mut ui_bridge,
        event_rx,
        cache_refresh_rx,
        config.keymap_style,
        &send_pipeline,
        console_gate.clone(),
        config.log_file_path.clone(),
        config.log_window_max_lines,
        llm_provider,
    )
    .await?;

    if let Some(handle) = history_handle {
        handle.abort();
        let _ = handle.await;
    }

    event_stream.stop().await;
    send_pipeline.stop().await;
    let cache_manager =
        Arc::try_unwrap(cache_manager).expect("cache manager still shared during shutdown");
    cache_manager.shutdown().await;
    bootstrap.shutdown().await;
    info!("shutdown complete");
    Ok(())
}

struct HistorySyncStats {
    dialog_count: usize,
    message_count: usize,
}

async fn sync_dialogs_and_history(
    client: &TelegramClient,
    cache_manager: &CacheManager,
    dialogs: Vec<telegram_llm_core::telegram::DialogSnapshot>,
    history_limit: usize,
    cache_refresh_tx: mpsc::UnboundedSender<()>,
) -> telegram_llm_core::telegram::Result<HistorySyncStats> {
    let dialog_count = dialogs.len();
    let mut message_count = 0;

    for dialog in dialogs {
        let summary = dialog.summary;
        let chat_id = summary.chat_id;
        let title = summary.title.clone();
        if history_limit == 0 {
            continue;
        }

        match fetch_recent_messages(client, dialog.peer, history_limit).await {
            Ok(messages) => {
                message_count += messages.len();
                for message in messages {
                    cache_manager.apply_event(&DomainEvent::MessageNew(message));
                }
                let _ = cache_refresh_tx.send(());
            }
            Err(err) => {
                warn!(
                    chat_id = chat_id.0,
                    title = %title,
                    error = %err,
                    "failed to fetch message history for chat"
                );
            }
        }
    }

    Ok(HistorySyncStats {
        dialog_count,
        message_count,
    })
}

fn effective_history_limit(history_per_chat: usize, cache_max_messages_per_chat: usize) -> usize {
    if cache_max_messages_per_chat == 0 {
        history_per_chat
    } else {
        history_per_chat.min(cache_max_messages_per_chat)
    }
}

fn init_tracing(config: &AppConfig) -> Result<ConsoleLogGate, Box<dyn std::error::Error>> {
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
    let console_gate = ConsoleLogGate::new();
    match config.log_format {
        LogFormat::Plain => {
            let stdout_timer = build_timer();
            let file_timer = build_timer();
            let error_timer = build_timer();
            let stdout_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_writer(console_gate.make_writer())
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
    Ok(console_gate)
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

#[derive(Clone)]
pub struct ConsoleLogGate {
    enabled: Arc<AtomicBool>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl ConsoleLogGate {
    fn new() -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(true)),
            writer: Arc::new(Mutex::new(Box::new(io::stdout()))),
        }
    }

    #[cfg(test)]
    fn with_writer(writer: Box<dyn Write + Send>) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(true)),
            writer: Arc::new(Mutex::new(writer)),
        }
    }

    fn make_writer(&self) -> GatedWriter {
        GatedWriter {
            enabled: self.enabled.clone(),
            writer: self.writer.clone(),
        }
    }

    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Relaxed);
    }

    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Relaxed);
    }
}

#[derive(Clone)]
struct GatedWriter {
    enabled: Arc<AtomicBool>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for GatedWriter {
    type Writer = GatedWriterGuard<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        GatedWriterGuard {
            enabled: self.enabled.clone(),
            guard: self.writer.lock().unwrap(),
        }
    }
}

struct GatedWriterGuard<'a> {
    enabled: Arc<AtomicBool>,
    guard: MutexGuard<'a, Box<dyn Write + Send>>,
}

impl Write for GatedWriterGuard<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Ok(buf.len());
        }
        self.guard.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Ok(());
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tracing_subscriber::fmt::MakeWriter;

    struct BufferWriter {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl Write for BufferWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.buffer.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn console_log_gate_suppresses_output_when_disabled() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer = BufferWriter {
            buffer: buffer.clone(),
        };
        let gate = ConsoleLogGate::with_writer(Box::new(writer));
        let make_writer = gate.make_writer();

        {
            let mut guard = make_writer.make_writer();
            guard.write_all(b"visible").unwrap();
        }
        assert_eq!(buffer.lock().unwrap().as_slice(), b"visible");

        gate.disable();
        {
            let mut guard = make_writer.make_writer();
            guard.write_all(b"hidden").unwrap();
        }
        assert_eq!(buffer.lock().unwrap().as_slice(), b"visible");
    }
}
