use std::path::PathBuf;
use std::sync::Arc;

use grammers_client::{Client, ClientConfiguration, UpdatesConfiguration};
use grammers_mtsender::{ConnectionParams, SenderPool, SenderPoolHandle};
use grammers_session::storages::SqliteSession;
use grammers_session::updates::UpdatesLike;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::telegram::auth::{AuthFlow, GrammersAuthClient};
use crate::telegram::error::Result;
use crate::telegram::updates::{spawn_telegram_update_pump, take_updates, UpdatePump};

#[derive(Debug, Clone)]
pub struct UpdatesConfig {
    pub catch_up: bool,
    pub update_queue_limit: Option<usize>,
}

impl Default for UpdatesConfig {
    fn default() -> Self {
        Self {
            catch_up: false,
            update_queue_limit: Some(100),
        }
    }
}

pub struct TelegramConfig {
    pub api_id: i32,
    pub api_hash: String,
    pub session_path: PathBuf,
    pub updates: UpdatesConfig,
    pub flood_sleep_threshold: u32,
    pub connection_params: ConnectionParams,
    pub qr_except_ids: Vec<i64>,
}

impl TelegramConfig {
    pub fn new(api_id: i32, api_hash: impl Into<String>, session_path: impl Into<PathBuf>) -> Self {
        Self {
            api_id,
            api_hash: api_hash.into(),
            session_path: session_path.into(),
            updates: UpdatesConfig::default(),
            flood_sleep_threshold: 60,
            connection_params: ConnectionParams::default(),
            qr_except_ids: Vec::new(),
        }
    }
}

pub struct TelegramBootstrap {
    client: Client,
    sender_handle: SenderPoolHandle,
    runner: JoinHandle<()>,
    updates: Option<mpsc::UnboundedReceiver<UpdatesLike>>,
    api_id: i32,
    api_hash: String,
    qr_except_ids: Vec<i64>,
    updates_config: UpdatesConfig,
}

impl TelegramBootstrap {
    pub async fn connect(config: TelegramConfig) -> Result<Self> {
        let TelegramConfig {
            api_id,
            api_hash,
            session_path,
            updates: updates_config,
            flood_sleep_threshold,
            connection_params,
            qr_except_ids,
        } = config;

        if let Some(parent) = session_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let session = Arc::new(SqliteSession::open(&session_path)?);
        let pool = SenderPool::with_configuration(Arc::clone(&session), api_id, connection_params);

        let client = Client::with_configuration(
            &pool,
            ClientConfiguration {
                flood_sleep_threshold,
            },
        );
        let updates = pool.updates;
        let sender_handle = pool.handle.clone();
        let runner = tokio::spawn(pool.runner.run());

        Ok(Self {
            client,
            sender_handle,
            runner,
            updates: Some(updates),
            api_id,
            api_hash,
            qr_except_ids,
            updates_config,
        })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn auth_flow(&self) -> AuthFlow<GrammersAuthClient> {
        AuthFlow::new(
            GrammersAuthClient::new(self.client.clone()),
            self.api_id,
            self.api_hash.clone(),
            self.qr_except_ids.clone(),
        )
    }

    pub fn spawn_update_pump(
        &mut self,
        buffer: usize,
    ) -> Result<UpdatePump<grammers_client::Update, grammers_mtsender::InvocationError>> {
        let updates = take_updates(&mut self.updates)?;
        Ok(spawn_telegram_update_pump(
            &self.client,
            updates,
            self.updates_config.clone().into(),
            buffer,
        ))
    }

    pub async fn shutdown(self) {
        let _ = self.sender_handle.quit();
        let _ = self.runner.await;
    }
}

impl From<UpdatesConfig> for UpdatesConfiguration {
    fn from(config: UpdatesConfig) -> Self {
        UpdatesConfiguration {
            catch_up: config.catch_up,
            update_queue_limit: config.update_queue_limit,
        }
    }
}
