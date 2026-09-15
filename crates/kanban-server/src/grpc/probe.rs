//! 只在 Host 测试中计数真实 adapter 调用；生产构建不保留观测状态。
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Default)]
pub(crate) struct SourceProbe {
    business: AtomicUsize,
    queries: Mutex<BTreeMap<String, usize>>,
    query_runtime: Mutex<Option<crate::grpc::query::QueryProbe>>,
}

impl SourceProbe {
    pub(crate) fn business(&self) {
        self.business.fetch_add(1, Ordering::Relaxed);
    }
    pub(crate) fn business_calls(&self) -> usize {
        self.business.load(Ordering::Relaxed)
    }

    pub(crate) fn attach_query_runtime(&self, runtime: crate::grpc::query::QueryProbe) {
        *self.query_runtime.lock().unwrap() = Some(runtime);
    }

    pub(crate) fn query_resources(&self) -> Option<(usize, usize, usize)> {
        self.query_runtime
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|runtime| runtime.resources())
    }

    pub(crate) fn query(&self, scope: &str) {
        *self
            .queries
            .lock()
            .unwrap()
            .entry(scope.into())
            .or_default() += 1;
    }

    pub(crate) fn queries(&self) -> BTreeMap<String, usize> {
        self.queries.lock().unwrap().clone()
    }
}
