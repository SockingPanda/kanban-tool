//! 不启动 Host 或 stdio 子进程；直接约束生产调度边界的占位、取消和超时。
use super::{McpError, run_limited};
use crate::errors::{Failure, OperationStatus};
use std::{
    cell::Cell,
    future::{Future, pending, poll_fn, ready},
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::sync::{Semaphore, oneshot};

struct PendingCall<'a> {
    polls: &'a Cell<usize>,
    dropped: &'a Cell<bool>,
}

impl Future for PendingCall<'_> {
    type Output = Result<(), McpError>;

    fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
        self.polls.set(self.polls.get() + 1);
        Poll::Pending
    }
}

impl Drop for PendingCall<'_> {
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}

#[tokio::test(start_paused = true)]
async fn overlapping_call_is_busy_without_starting_its_handler() {
    let slots = Semaphore::new(1);
    let (release, released) = oneshot::channel::<()>();
    let mut first = Box::pin(run_limited(
        &slots,
        Duration::from_secs(60),
        pending::<()>(),
        || async {
            released.await.expect("release first call");
            Ok::<_, McpError>(())
        },
    ));
    // 显式 poll 到首请求持有 permit；不依赖 accept、sleep 或两个进程的调度速度。
    poll_fn(|context| {
        assert!(first.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
    assert_eq!(slots.available_permits(), 0);

    let started = Cell::new(false);
    let error = run_limited(&slots, Duration::from_secs(60), pending::<()>(), || {
        started.set(true);
        ready(Ok::<_, McpError>(()))
    })
    .await
    .expect_err("second call must be rejected before handler construction");
    assert!(!started.get());
    let failure = Failure::from_internal(&error).expect("typed scheduling failure");
    assert_eq!(failure.code, "busy");
    assert_eq!(failure.operation_status, OperationStatus::NotStarted);
    let wire = serde_json::to_value(failure.into_tool_result(false, "task_create")).unwrap();
    assert_eq!(wire["isError"], true);
    assert_eq!(
        wire["_meta"][crate::metadata::ERROR_KEY]["retry_safe"],
        true
    );
    assert_eq!(
        slots.available_permits(),
        0,
        "rejection cannot release another call's permit"
    );

    release.send(()).unwrap();
    first.await.unwrap();
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test(start_paused = true)]
async fn timeout_waits_for_its_deadline_drops_work_and_releases_capacity() {
    let slots = Semaphore::new(1);
    let polls = Cell::new(0);
    let dropped = Cell::new(false);
    let mut request = Box::pin(run_limited(
        &slots,
        Duration::from_millis(100),
        pending::<()>(),
        || PendingCall {
            polls: &polls,
            dropped: &dropped,
        },
    ));
    poll_fn(|context| {
        assert!(request.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
    assert!(polls.get() > 0);
    assert_eq!(slots.available_permits(), 0);
    tokio::time::advance(Duration::from_millis(99)).await;
    poll_fn(|context| {
        assert!(
            request.as_mut().poll(context).is_pending(),
            "deadline 前不能超时"
        );
        Poll::Ready(())
    })
    .await;
    assert!(!dropped.get());
    tokio::time::advance(Duration::from_millis(1)).await;
    let error = request
        .await
        .expect_err("deadline must cancel pending work");
    let failure = Failure::from_internal(&error).unwrap();
    assert_eq!(failure.code, "timeout");
    assert_eq!(failure.operation_status, OperationStatus::Unknown);
    assert!(dropped.get());
    assert_eq!(slots.available_permits(), 1);
    assert_eq!(
        run_limited(&slots, Duration::from_secs(1), pending::<()>(), || {
            ready(Ok::<_, McpError>(7))
        })
        .await
        .unwrap(),
        7
    );
}

#[tokio::test(start_paused = true)]
async fn cancellation_drops_in_flight_work_and_releases_capacity() {
    let slots = Semaphore::new(1);
    let polls = Cell::new(0);
    let dropped = Cell::new(false);
    let (cancel, cancelled) = oneshot::channel::<()>();
    let mut request = Box::pin(run_limited(
        &slots,
        Duration::from_secs(60),
        async { cancelled.await.expect("cancellation signal") },
        || PendingCall {
            polls: &polls,
            dropped: &dropped,
        },
    ));
    poll_fn(|context| {
        assert!(request.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
    assert_eq!(slots.available_permits(), 0);
    cancel.send(()).unwrap();
    let error = request.await.expect_err("cancelled call must not succeed");
    let failure = Failure::from_internal(&error).unwrap();
    assert_eq!(failure.code, "cancelled");
    assert_eq!(failure.operation_status, OperationStatus::Unknown);
    assert!(dropped.get());
    assert_eq!(slots.available_permits(), 1);
}
