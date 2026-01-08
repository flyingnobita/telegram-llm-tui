use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use grammers_mtsender::{InvocationError, RpcError};
use grammers_session::defs::{PeerAuth, PeerId, PeerRef};
use telegram_llm_core::telegram::send::{SendError, SendTransport};
use telegram_llm_core::telegram::{
    spawn_send_pipeline, MessageId, SendEnqueueError, SendPipelineConfig, SendRequest, SendResult,
    SendStatus,
};

#[derive(Clone)]
struct MockTransport {
    responses: Arc<Mutex<VecDeque<Result<SendResult, SendError>>>>,
}

impl MockTransport {
    fn new(responses: Vec<Result<SendResult, SendError>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses.into())),
        }
    }
}

#[async_trait]
impl SendTransport for MockTransport {
    async fn execute(&self, _request: &SendRequest) -> Result<SendResult, SendError> {
        let mut guard = self.responses.lock().unwrap();
        guard.pop_front().expect("missing mock transport response")
    }
}

fn test_peer() -> PeerRef {
    PeerRef {
        id: PeerId::user(123),
        auth: PeerAuth::default(),
    }
}

fn send_request() -> SendRequest {
    SendRequest::SendText {
        peer: test_peer(),
        text: "hello".to_string(),
        reply_to: None,
    }
}

async fn wait_for_status<F>(
    status: &mut tokio::sync::watch::Receiver<SendStatus>,
    predicate: F,
) -> SendStatus
where
    F: Fn(&SendStatus) -> bool,
{
    loop {
        let current = status.borrow().clone();
        if predicate(&current) {
            return current;
        }
        if status.changed().await.is_err() {
            return status.borrow().clone();
        }
    }
}

#[tokio::test(start_paused = true)]
async fn retries_on_rate_limit_then_succeeds() {
    let rpc_error = RpcError {
        code: 420,
        name: "FLOOD_WAIT".to_string(),
        value: Some(1),
        caused_by: None,
    };
    let responses = vec![
        Err(SendError::Invocation(InvocationError::Rpc(rpc_error))),
        Ok(SendResult::MessageSent {
            message_id: MessageId(77),
        }),
    ];
    let transport = MockTransport::new(responses);
    let config = SendPipelineConfig {
        queue_limit: 4,
        max_retry_attempts: Some(3),
        retry_base_delay: Duration::from_millis(10),
        retry_max_delay: Duration::from_millis(1000),
    };
    let pipeline = spawn_send_pipeline(transport, config);

    let ticket = pipeline.enqueue(send_request()).expect("enqueue");
    let mut status_rx = ticket.status;

    tokio::time::advance(Duration::from_millis(1)).await;
    let queued = wait_for_status(&mut status_rx, |status| {
        matches!(
            status,
            SendStatus::Queued {
                attempt: 1,
                next_retry_in: Some(delay)
            } if *delay == Duration::from_secs(1)
        )
    })
    .await;

    assert!(matches!(
        queued,
        SendStatus::Queued {
            attempt: 1,
            next_retry_in: Some(_)
        }
    ));

    tokio::time::advance(Duration::from_secs(1)).await;
    let sent = wait_for_status(&mut status_rx, |status| {
        matches!(status, SendStatus::Sent(_))
    })
    .await;

    assert!(matches!(
        sent,
        SendStatus::Sent(SendResult::MessageSent {
            message_id: MessageId(77)
        })
    ));

    pipeline.stop().await;
}

#[tokio::test(start_paused = true)]
async fn fails_after_max_retry_attempts() {
    let error_one = SendError::Invocation(InvocationError::Io(std::io::Error::other("boom")));
    let error_two = SendError::Invocation(InvocationError::Io(std::io::Error::other("boom")));
    let responses = vec![Err(error_one), Err(error_two)];
    let transport = MockTransport::new(responses);
    let config = SendPipelineConfig {
        queue_limit: 2,
        max_retry_attempts: Some(2),
        retry_base_delay: Duration::from_millis(5),
        retry_max_delay: Duration::from_millis(5),
    };
    let pipeline = spawn_send_pipeline(transport, config);

    let ticket = pipeline.enqueue(send_request()).expect("enqueue");
    let mut status_rx = ticket.status;

    tokio::time::advance(Duration::from_millis(1)).await;
    let _ = wait_for_status(&mut status_rx, |status| {
        matches!(
            status,
            SendStatus::Queued {
                attempt: 1,
                next_retry_in: Some(_)
            }
        )
    })
    .await;

    tokio::time::advance(Duration::from_millis(5)).await;
    let failed = wait_for_status(&mut status_rx, |status| {
        matches!(status, SendStatus::Failed(_))
    })
    .await;

    match failed {
        SendStatus::Failed(failure) => {
            assert_eq!(failure.attempts, 2);
            assert!(failure.retryable);
        }
        other => panic!("expected failed status, got {other:?}"),
    }

    pipeline.stop().await;
}

#[tokio::test]
async fn rejects_enqueue_when_queue_full() {
    let responses = vec![Ok(SendResult::MessageSent {
        message_id: MessageId(1),
    })];
    let transport = MockTransport::new(responses);
    let config = SendPipelineConfig {
        queue_limit: 1,
        max_retry_attempts: Some(1),
        retry_base_delay: Duration::from_millis(1),
        retry_max_delay: Duration::from_millis(1),
    };
    let pipeline = spawn_send_pipeline(transport, config);

    let _first = pipeline.enqueue(send_request()).expect("first enqueue");
    let second = pipeline.enqueue(send_request());

    assert!(matches!(second, Err(SendEnqueueError::QueueFull)));

    pipeline.stop().await;
}

#[tokio::test(start_paused = true)]
async fn invalid_message_ids_fail_without_retry() {
    let error = SendError::InvalidMessageId {
        field: "message_id",
        value: i64::from(i32::MAX) + 1,
    };
    let responses = vec![Err(error)];
    let transport = MockTransport::new(responses);
    let config = SendPipelineConfig {
        queue_limit: 2,
        max_retry_attempts: Some(3),
        retry_base_delay: Duration::from_millis(5),
        retry_max_delay: Duration::from_millis(5),
    };
    let pipeline = spawn_send_pipeline(transport, config);

    let ticket = pipeline.enqueue(send_request()).expect("enqueue");
    let mut status_rx = ticket.status;

    tokio::time::advance(Duration::from_millis(1)).await;
    let failed = wait_for_status(&mut status_rx, |status| {
        matches!(status, SendStatus::Failed(_))
    })
    .await;

    match failed {
        SendStatus::Failed(failure) => {
            assert_eq!(failure.attempts, 1);
            assert!(!failure.retryable);
        }
        other => panic!("expected failed status, got {other:?}"),
    }

    pipeline.stop().await;
}
