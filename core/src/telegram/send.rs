use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use grammers_client::types::InputMessage;
use grammers_client::Client;
use grammers_mtsender::{InvocationError, RpcError};
use grammers_session::defs::PeerRef;
use tokio::sync::{mpsc, watch, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::{sleep_until, Instant};
use tracing::{info, warn};

use crate::telegram::events::MessageId;

#[derive(Debug, Clone)]
pub struct SendPipelineConfig {
    pub queue_limit: usize,
    pub max_retry_attempts: Option<u32>,
    pub retry_base_delay: Duration,
    pub retry_max_delay: Duration,
}

impl Default for SendPipelineConfig {
    fn default() -> Self {
        Self {
            queue_limit: 256,
            max_retry_attempts: None,
            retry_base_delay: Duration::from_millis(500),
            retry_max_delay: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SendId(pub u64);

#[derive(Debug, Clone)]
pub enum SendRequest {
    SendText {
        peer: PeerRef,
        text: String,
        reply_to: Option<MessageId>,
    },
    EditText {
        peer: PeerRef,
        message_id: MessageId,
        text: String,
    },
    DeleteMessage {
        peer: PeerRef,
        message_id: MessageId,
    },
}

impl SendRequest {
    fn kind(&self) -> &'static str {
        match self {
            Self::SendText { .. } => "send_text",
            Self::EditText { .. } => "edit_text",
            Self::DeleteMessage { .. } => "delete_message",
        }
    }

    fn peer_id(&self) -> i64 {
        match self {
            Self::SendText { peer, .. }
            | Self::EditText { peer, .. }
            | Self::DeleteMessage { peer, .. } => peer.id.bot_api_dialog_id(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendResult {
    MessageSent {
        message_id: MessageId,
    },
    MessageEdited {
        message_id: MessageId,
    },
    MessageDeleted {
        message_id: MessageId,
        deleted_count: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendStatus {
    Queued {
        attempt: u32,
        next_retry_in: Option<Duration>,
    },
    Sending {
        attempt: u32,
    },
    Sent(SendResult),
    Failed(SendFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendFailure {
    pub error: String,
    pub attempts: u32,
    pub retryable: bool,
}

#[derive(Debug)]
pub struct SendTicket {
    pub id: SendId,
    pub status: watch::Receiver<SendStatus>,
}

#[derive(Debug, thiserror::Error)]
pub enum SendEnqueueError {
    #[error("send queue is full")]
    QueueFull,
    #[error("send pipeline is closed")]
    Closed,
}

#[derive(Debug, thiserror::Error)]
pub enum SendError {
    #[error("invalid message id for {field}: {value}")]
    InvalidMessageId { field: &'static str, value: i64 },
    #[error("telegram invocation error: {0}")]
    Invocation(#[from] InvocationError),
}

#[async_trait]
pub trait SendTransport: Send + Sync + 'static {
    async fn execute(&self, request: &SendRequest) -> Result<SendResult, SendError>;
}

#[derive(Clone)]
pub struct GrammersSendTransport {
    client: Client,
}

impl GrammersSendTransport {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl SendTransport for GrammersSendTransport {
    async fn execute(&self, request: &SendRequest) -> Result<SendResult, SendError> {
        match request {
            SendRequest::SendText {
                peer,
                text,
                reply_to,
            } => {
                let reply_to = match reply_to {
                    Some(message_id) => Some(message_id_i32(*message_id, "reply_to")?),
                    None => None,
                };
                let input = InputMessage::new().text(text.clone()).reply_to(reply_to);
                let message = self.client.send_message(*peer, input).await?;
                Ok(SendResult::MessageSent {
                    message_id: MessageId(message.id() as i64),
                })
            }
            SendRequest::EditText {
                peer,
                message_id,
                text,
            } => {
                let message_id_value = message_id_i32(*message_id, "message_id")?;
                let input = InputMessage::new().text(text.clone());
                self.client
                    .edit_message(*peer, message_id_value, input)
                    .await?;
                Ok(SendResult::MessageEdited {
                    message_id: *message_id,
                })
            }
            SendRequest::DeleteMessage { peer, message_id } => {
                let message_id_value = message_id_i32(*message_id, "message_id")?;
                let deleted = self
                    .client
                    .delete_messages(*peer, &[message_id_value])
                    .await?;
                Ok(SendResult::MessageDeleted {
                    message_id: *message_id,
                    deleted_count: deleted,
                })
            }
        }
    }
}

pub fn spawn_send_pipeline<T>(transport: T, config: SendPipelineConfig) -> SendPipeline
where
    T: SendTransport,
{
    spawn_send_pipeline_with_transport(Arc::new(transport), config)
}

pub fn spawn_grammers_send_pipeline(client: Client, config: SendPipelineConfig) -> SendPipeline {
    spawn_send_pipeline(GrammersSendTransport::new(client), config)
}

pub fn spawn_send_pipeline_with_transport(
    transport: Arc<dyn SendTransport>,
    config: SendPipelineConfig,
) -> SendPipeline {
    let (tx, rx) = mpsc::channel(config.queue_limit.max(1));
    let (stop_tx, stop_rx) = watch::channel(false);
    let permits = Arc::new(Semaphore::new(config.queue_limit.max(1)));
    let id_counter = Arc::new(AtomicU64::new(1));

    let join = tokio::spawn(run_send_worker(rx, stop_rx, transport, config));

    SendPipeline {
        tx,
        stop_tx,
        join,
        permits,
        id_counter,
    }
}

pub struct SendPipeline {
    tx: mpsc::Sender<SendCommand>,
    stop_tx: watch::Sender<bool>,
    join: JoinHandle<()>,
    permits: Arc<Semaphore>,
    id_counter: Arc<AtomicU64>,
}

impl SendPipeline {
    pub fn enqueue(&self, request: SendRequest) -> Result<SendTicket, SendEnqueueError> {
        let permit = self
            .permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| SendEnqueueError::QueueFull)?;
        let id = SendId(self.id_counter.fetch_add(1, AtomicOrdering::Relaxed));
        let (status_tx, status_rx) = watch::channel(SendStatus::Queued {
            attempt: 0,
            next_retry_in: None,
        });
        let command = SendCommand::Enqueue {
            id,
            request,
            status: status_tx,
            permit,
        };
        match self.tx.try_send(command) {
            Ok(()) => Ok(SendTicket {
                id,
                status: status_rx,
            }),
            Err(mpsc::error::TrySendError::Full(command)) => {
                drop(command);
                Err(SendEnqueueError::QueueFull)
            }
            Err(mpsc::error::TrySendError::Closed(command)) => {
                drop(command);
                Err(SendEnqueueError::Closed)
            }
        }
    }

    pub async fn stop(self) {
        let _ = self.stop_tx.send(true);
        let _ = self.join.await;
    }
}

#[derive(Debug)]
enum SendCommand {
    Enqueue {
        id: SendId,
        request: SendRequest,
        status: watch::Sender<SendStatus>,
        permit: OwnedSemaphorePermit,
    },
}

#[derive(Debug)]
struct QueueItem {
    id: SendId,
    request: SendRequest,
    status: watch::Sender<SendStatus>,
    attempts: u32,
    next_attempt: Instant,
    sequence: u64,
    _permit: OwnedSemaphorePermit,
}

impl PartialEq for QueueItem {
    fn eq(&self, other: &Self) -> bool {
        self.next_attempt == other.next_attempt && self.sequence == other.sequence
    }
}

impl Eq for QueueItem {}

impl PartialOrd for QueueItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueueItem {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .next_attempt
            .cmp(&self.next_attempt)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

async fn run_send_worker(
    mut rx: mpsc::Receiver<SendCommand>,
    mut stop_rx: watch::Receiver<bool>,
    transport: Arc<dyn SendTransport>,
    config: SendPipelineConfig,
) {
    let mut queue: BinaryHeap<QueueItem> = BinaryHeap::new();
    let mut sequence = 0u64;

    loop {
        let next_deadline = queue.peek().map(|item| item.next_attempt);
        let sleep_deadline = next_deadline.unwrap_or_else(Instant::now);

        tokio::select! {
            _ = stop_rx.changed() => {
                break;
            }
            command = rx.recv() => {
                let Some(command) = command else {
                    break;
                };
                match command {
                    SendCommand::Enqueue { id, request, status, permit } => {
                        sequence = sequence.wrapping_add(1);
                        let _ = status.send(SendStatus::Queued { attempt: 0, next_retry_in: None });
                        queue.push(QueueItem {
                            id,
                            request,
                            status,
                            attempts: 0,
                            next_attempt: Instant::now(),
                            sequence,
                            _permit: permit,
                        });
                    }
                }
            }
            _ = sleep_until(sleep_deadline), if next_deadline.is_some() => {
                let now = Instant::now();
                while queue.peek().is_some_and(|item| item.next_attempt <= now) {
                    let Some(item) = queue.pop() else {
                        break;
                    };
                    process_queue_item(item, &transport, &config, &mut queue, &mut sequence).await;
                }
            }
        }
    }
}

async fn process_queue_item(
    mut item: QueueItem,
    transport: &Arc<dyn SendTransport>,
    config: &SendPipelineConfig,
    queue: &mut BinaryHeap<QueueItem>,
    sequence: &mut u64,
) {
    let attempt = item.attempts.saturating_add(1);
    item.attempts = attempt;
    let _ = item.status.send(SendStatus::Sending { attempt });
    info!(
        send_id = item.id.0,
        attempt,
        request = item.request.kind(),
        peer_id = item.request.peer_id(),
        "sending telegram request"
    );

    match transport.execute(&item.request).await {
        Ok(result) => {
            let _ = item.status.send(SendStatus::Sent(result.clone()));
            info!(
                send_id = item.id.0,
                attempt,
                request = item.request.kind(),
                peer_id = item.request.peer_id(),
                "telegram request sent"
            );
        }
        Err(error) => {
            let decision = retry_decision(&error, attempt, config);
            match decision {
                RetryDecision::RetryAfter(delay) => {
                    if exceeded_max_attempts(attempt, config.max_retry_attempts) {
                        let _ = item.status.send(SendStatus::Failed(SendFailure {
                            error: error.to_string(),
                            attempts: attempt,
                            retryable: true,
                        }));
                        warn!(
                            send_id = item.id.0,
                            attempt,
                            request = item.request.kind(),
                            peer_id = item.request.peer_id(),
                            error = %error,
                            "send pipeline exceeded retry attempts"
                        );
                        return;
                    }
                    let _ = item.status.send(SendStatus::Queued {
                        attempt,
                        next_retry_in: Some(delay),
                    });
                    warn!(
                        send_id = item.id.0,
                        attempt,
                        request = item.request.kind(),
                        peer_id = item.request.peer_id(),
                        delay_ms = delay.as_millis(),
                        error = %error,
                        "retrying telegram send request"
                    );
                    item.next_attempt = Instant::now() + delay;
                    *sequence = sequence.wrapping_add(1);
                    item.sequence = *sequence;
                    queue.push(item);
                }
                RetryDecision::Fail { retryable } => {
                    let _ = item.status.send(SendStatus::Failed(SendFailure {
                        error: error.to_string(),
                        attempts: attempt,
                        retryable,
                    }));
                    warn!(
                        send_id = item.id.0,
                        attempt,
                        request = item.request.kind(),
                        peer_id = item.request.peer_id(),
                        error = %error,
                        "failed to send telegram request"
                    );
                }
            }
        }
    }
}

fn exceeded_max_attempts(attempt: u32, max_attempts: Option<u32>) -> bool {
    match max_attempts {
        Some(max) => attempt >= max,
        None => false,
    }
}

enum RetryDecision {
    RetryAfter(Duration),
    Fail { retryable: bool },
}

fn retry_decision(error: &SendError, attempt: u32, config: &SendPipelineConfig) -> RetryDecision {
    match error {
        SendError::InvalidMessageId { .. } => RetryDecision::Fail { retryable: false },
        SendError::Invocation(err) => match err {
            InvocationError::Rpc(rpc) => {
                if let Some(delay) = rate_limit_delay(rpc) {
                    return RetryDecision::RetryAfter(delay);
                }
                if rpc.code >= 500 {
                    return RetryDecision::RetryAfter(backoff_delay(attempt, config));
                }
                RetryDecision::Fail { retryable: false }
            }
            InvocationError::Io(_)
            | InvocationError::Transport(_)
            | InvocationError::Deserialize(_) => {
                RetryDecision::RetryAfter(backoff_delay(attempt, config))
            }
            InvocationError::Dropped | InvocationError::InvalidDc => {
                RetryDecision::RetryAfter(backoff_delay(attempt, config))
            }
            InvocationError::Authentication(_) => RetryDecision::Fail { retryable: false },
        },
    }
}

fn rate_limit_delay(rpc: &RpcError) -> Option<Duration> {
    match rpc.name.as_str() {
        "FLOOD_WAIT" | "SLOWMODE_WAIT" | "FLOOD_PREMIUM_WAIT" => rpc
            .value
            .map(|seconds| Duration::from_secs(seconds as u64))
            .filter(|delay| !delay.is_zero()),
        _ => None,
    }
}

fn backoff_delay(attempt: u32, config: &SendPipelineConfig) -> Duration {
    let base_ms = config.retry_base_delay.as_millis() as u64;
    let max_ms = config.retry_max_delay.as_millis() as u64;
    if base_ms == 0 || max_ms == 0 {
        return Duration::from_millis(0);
    }
    let mut delay_ms = base_ms;
    let mut step = 1;
    while step < attempt {
        delay_ms = delay_ms.saturating_mul(2);
        if delay_ms >= max_ms {
            delay_ms = max_ms;
            break;
        }
        step += 1;
    }
    Duration::from_millis(delay_ms)
}

fn message_id_i32(message_id: MessageId, field: &'static str) -> Result<i32, SendError> {
    i32::try_from(message_id.0).map_err(|_| SendError::InvalidMessageId {
        field,
        value: message_id.0,
    })
}
