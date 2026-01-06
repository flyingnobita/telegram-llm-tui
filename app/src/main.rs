mod config;
mod prompt;

use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use telegram_llm_core::telegram::{AuthResult, QrLoginResult, TelegramBootstrap, TelegramConfig};
use tracing::{info, warn};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;

use crate::config::AppConfig;
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

    let telegram_config =
        TelegramConfig::new(config.api_id, config.api_hash, config.session_path.clone());

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

    info!("starting update pump");
    let mut pump = bootstrap.spawn_update_pump(config.update_buffer)?;

    tokio::select! {
        _ = async {
            while let Some(event) = pump.receiver().recv().await {
                match event {
                    telegram_llm_core::telegram::UpdateEvent::Update(update) => {
                        info!(?update, "received update");
                    }
                    telegram_llm_core::telegram::UpdateEvent::Error(err) => {
                        warn!(error = %err, "update pump error");
                        break;
                    }
                }
            }
        } => {}
        _ = tokio::signal::ctrl_c() => {
            info!("shutdown requested");
        }
    }

    pump.stop().await;
    bootstrap.shutdown().await;
    info!("shutdown complete");
    Ok(())
}

fn init_tracing(config: &AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    let log_path = &config.error_log_path;
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level_filter_directive(config.log_level)));
    let stdout_layer = tracing_subscriber::fmt::layer().with_filter(filter);
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::sync::Mutex::new(file))
        .with_ansi(false)
        .with_filter(tracing_subscriber::filter::LevelFilter::ERROR);

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .init();
    Ok(())
}

fn level_filter_directive(level: LevelFilter) -> &'static str {
    match level {
        LevelFilter::ERROR => "error",
        LevelFilter::WARN => "warn",
        LevelFilter::INFO => "info",
        LevelFilter::DEBUG => "debug",
        LevelFilter::TRACE => "trace",
        LevelFilter::OFF => "off",
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
