//! 单 host 的写入互斥与实时读取唤醒。
//!
//! 唤醒只表示写入临界区已退出。失败或取消也可能唤醒订阅者；只有已提交的
//! application 一致快照能进入投影。revision 不是客户端的投影 cursor。

use tokio::sync::{Mutex, MutexGuard, watch};

pub(crate) struct MutationGate {
    mutex: Mutex<()>,
    revision: watch::Sender<u64>,
}

impl MutationGate {
    pub(crate) fn new() -> Self {
        let (revision, _) = watch::channel(0);
        Self {
            mutex: Mutex::new(()),
            revision,
        }
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.revision.subscribe()
    }

    /// 读取 fence 不发通知，防止读取触发自身无限刷新。
    pub(crate) async fn read(&self) -> MutexGuard<'_, ()> {
        self.mutex.lock().await
    }

    pub(crate) async fn lock(&self) -> MutationGuard<'_> {
        let guard = self.mutex.lock().await;
        MutationGuard {
            guard: Some(guard),
            revision: &self.revision,
        }
    }
}

pub(crate) struct MutationGuard<'a> {
    guard: Option<MutexGuard<'a, ()>>,
    revision: &'a watch::Sender<u64>,
}

impl Drop for MutationGuard<'_> {
    fn drop(&mut self) {
        // 唤醒前释放锁。这里只提供提示，不发送事务结果或业务 payload。
        drop(self.guard.take());
        self.revision
            .send_modify(|revision| *revision = revision.wrapping_add(1));
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use super::*;

    #[tokio::test]
    async fn read_fence_is_silent() {
        let gate = MutationGate::new();
        let changes = gate.subscribe();
        drop(gate.read().await);
        assert!(!changes.has_changed().unwrap());
    }

    #[tokio::test]
    async fn no_notification_while_writer_holds_gate() {
        let gate = MutationGate::new();
        let mut changes = gate.subscribe();
        let guard = gate.lock().await;
        assert!(!changes.has_changed().unwrap());
        drop(guard);
        changes.changed().await.unwrap();
        assert_eq!(*changes.borrow_and_update(), 1);
        assert!(!changes.has_changed().unwrap());
    }

    #[tokio::test]
    async fn release_is_observed_even_before_wait_is_polled() {
        let gate = MutationGate::new();
        let mut changes = gate.subscribe();
        drop(gate.lock().await);
        tokio::time::timeout(Duration::from_secs(1), changes.changed())
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn many_writes_coalesce_without_an_event_queue() {
        let gate = MutationGate::new();
        let mut changes = gate.subscribe();
        for _ in 0..10_000 {
            drop(gate.lock().await);
        }
        changes.changed().await.unwrap();
        assert_eq!(*changes.borrow_and_update(), 10_000);
        assert!(!changes.has_changed().unwrap());
    }

    #[tokio::test]
    async fn every_subscriber_is_notified() {
        let gate = MutationGate::new();
        let mut first = gate.subscribe();
        let mut second = gate.subscribe();
        drop(gate.lock().await);
        first.changed().await.unwrap();
        second.changed().await.unwrap();
    }

    #[tokio::test]
    async fn no_subscribers_does_not_lose_future_notifications() {
        let gate = MutationGate::new();
        drop(gate.lock().await);
        let mut changes = gate.subscribe();
        assert_eq!(*changes.borrow_and_update(), 1);
        drop(gate.lock().await);
        changes.changed().await.unwrap();
        assert_eq!(*changes.borrow_and_update(), 2);
    }

    #[tokio::test]
    async fn failed_operation_wakes_without_fabricating_a_business_event() {
        let gate = MutationGate::new();
        let mut changes = gate.subscribe();
        let failed: Result<(), ()> = async {
            let _guard = gate.lock().await;
            Err(())
        }
        .await;
        assert!(failed.is_err());
        changes.changed().await.unwrap();
        // revision 没有业务内容。读者必须执行 application 一致读取。
        assert_eq!(*changes.borrow_and_update(), 1);
    }

    #[tokio::test]
    async fn cancelling_writer_releases_gate_and_wakes_reader() {
        let gate = Arc::new(MutationGate::new());
        let mut changes = gate.subscribe();
        let writer_gate = gate.clone();
        let (ready, acquired) = tokio::sync::oneshot::channel();
        let writer = tokio::spawn(async move {
            let _guard = writer_gate.lock().await;
            ready.send(()).unwrap();
            std::future::pending::<()>().await;
        });
        acquired.await.unwrap();
        writer.abort();
        assert!(writer.await.unwrap_err().is_cancelled());
        changes.changed().await.unwrap();
        let _next_writer = gate.lock().await;
    }
}
