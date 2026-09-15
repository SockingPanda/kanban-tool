//! 只在 Host 测试中计数真实 adapter 调用；生产构建不保留观测状态。
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tokio::sync::Semaphore;

#[derive(Default)]
pub(crate) struct SourceProbe {
    checks: Mutex<BTreeMap<String, usize>>,
    loads: Mutex<BTreeMap<String, usize>>,
    pause: Mutex<Option<Arc<CheckPause>>>,
}

impl SourceProbe {
    pub(crate) async fn check(&self, board: &str) {
        *self.checks.lock().unwrap().entry(board.into()).or_default() += 1;
        let pause = self.pause.lock().unwrap().take();
        if let Some(pause) = pause {
            pause.entered.add_permits(1);
            pause.release.acquire().await.unwrap().forget();
        }
    }

    pub(crate) fn load(&self, board: &str) {
        *self.loads.lock().unwrap().entry(board.into()).or_default() += 1;
    }

    pub(crate) fn checks(&self) -> BTreeMap<String, usize> {
        self.checks.lock().unwrap().clone()
    }

    pub(crate) fn loads(&self) -> BTreeMap<String, usize> {
        self.loads.lock().unwrap().clone()
    }

    pub(crate) fn pause_next_check(&self) -> Arc<CheckPause> {
        let pause = Arc::new(CheckPause {
            entered: Semaphore::new(0),
            release: Semaphore::new(0),
        });
        *self.pause.lock().unwrap() = Some(pause.clone());
        pause
    }
}

pub(crate) struct CheckPause {
    pub(crate) entered: Semaphore,
    pub(crate) release: Semaphore,
}
