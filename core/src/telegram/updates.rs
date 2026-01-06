use async_trait::async_trait;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use grammers_client::{Client, UpdatesConfiguration};
use grammers_session::updates::UpdatesLike;

use crate::telegram::error::{Result, TelegramError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateEvent<U, E> {
    Update(U),
    Error(E),
}

#[async_trait]
pub trait UpdateSource: Send + 'static {
    type Update: Send + 'static;
    type Error: Send + 'static;

    async fn next_update(&mut self) -> std::result::Result<Self::Update, Self::Error>;
}

pub struct GrammersUpdateSource {
    inner: grammers_client::client::updates::UpdateStream,
}

impl GrammersUpdateSource {
    pub fn new(
        client: &Client,
        updates: mpsc::UnboundedReceiver<UpdatesLike>,
        configuration: UpdatesConfiguration,
    ) -> Self {
        Self {
            inner: client.stream_updates(updates, configuration),
        }
    }
}

#[async_trait]
impl UpdateSource for GrammersUpdateSource {
    type Update = grammers_client::Update;
    type Error = grammers_mtsender::InvocationError;

    async fn next_update(&mut self) -> std::result::Result<Self::Update, Self::Error> {
        self.inner.next().await
    }
}

pub struct UpdatePump<U, E> {
    receiver: mpsc::Receiver<UpdateEvent<U, E>>,
    stop_tx: watch::Sender<bool>,
    join: JoinHandle<()>,
}

impl<U, E> UpdatePump<U, E> {
    pub fn receiver(&mut self) -> &mut mpsc::Receiver<UpdateEvent<U, E>> {
        &mut self.receiver
    }

    pub async fn stop(self) {
        let _ = self.stop_tx.send(true);
        let _ = self.join.await;
    }
}

pub fn spawn_update_pump<S>(mut source: S, buffer: usize) -> UpdatePump<S::Update, S::Error>
where
    S: UpdateSource,
{
    let (tx, rx) = mpsc::channel(buffer);
    let (stop_tx, mut stop_rx) = watch::channel(false);

    let join = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = stop_rx.changed() => {
                    break;
                }
                update = source.next_update() => {
                    match update {
                        Ok(update) => {
                            if tx.send(UpdateEvent::Update(update)).await.is_err() {
                                break;
                            }
                        }
                        Err(err) => {
                            let _ = tx.send(UpdateEvent::Error(err)).await;
                            break;
                        }
                    }
                }
            }
        }
    });

    UpdatePump {
        receiver: rx,
        stop_tx,
        join,
    }
}

pub fn spawn_telegram_update_pump(
    client: &Client,
    updates: mpsc::UnboundedReceiver<UpdatesLike>,
    configuration: UpdatesConfiguration,
    buffer: usize,
) -> UpdatePump<grammers_client::Update, grammers_mtsender::InvocationError> {
    let source = GrammersUpdateSource::new(client, updates, configuration);
    spawn_update_pump(source, buffer)
}

pub fn take_updates(
    updates: &mut Option<mpsc::UnboundedReceiver<UpdatesLike>>,
) -> Result<mpsc::UnboundedReceiver<UpdatesLike>> {
    updates
        .take()
        .ok_or(TelegramError::UpdatePumpUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct MockUpdateSource {
        queue: VecDeque<std::result::Result<&'static str, &'static str>>,
    }

    impl MockUpdateSource {
        fn new(items: Vec<std::result::Result<&'static str, &'static str>>) -> Self {
            Self {
                queue: items.into(),
            }
        }
    }

    #[async_trait]
    impl UpdateSource for MockUpdateSource {
        type Update = &'static str;
        type Error = &'static str;

        async fn next_update(&mut self) -> std::result::Result<Self::Update, Self::Error> {
            self.queue.pop_front().unwrap_or(Err("end"))
        }
    }

    #[tokio::test]
    async fn update_pump_forwards_events() {
        let source = MockUpdateSource::new(vec![Ok("one"), Ok("two"), Err("boom")]);
        let mut pump = spawn_update_pump(source, 4);

        let first = pump.receiver().recv().await;
        assert_eq!(first, Some(UpdateEvent::Update("one")));

        let second = pump.receiver().recv().await;
        assert_eq!(second, Some(UpdateEvent::Update("two")));

        let third = pump.receiver().recv().await;
        assert_eq!(third, Some(UpdateEvent::Error("boom")));

        pump.stop().await;
    }
}
