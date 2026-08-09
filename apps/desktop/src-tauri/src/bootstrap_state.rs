use serde::Serialize;

/// Bootstrap 操作的来源。一个 generation 同时最多允许一个操作运行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttemptKind {
    ProbeExisting,
    StartLocal,
}

/// 绑定后台操作结果的 generation/fence。
#[derive(Debug, Default)]
pub(crate) struct GenerationFence {
    generation: u64,
    in_flight: Option<(u64, AttemptKind)>,
    closed: bool,
}

impl GenerationFence {
    pub(crate) fn begin(&mut self, kind: AttemptKind) -> Option<u64> {
        if self.closed || self.in_flight.is_some() {
            return None;
        }
        self.generation = self.generation.saturating_add(1);
        self.in_flight = Some((self.generation, kind));
        Some(self.generation)
    }

    pub(crate) fn finish(&mut self, generation: u64, kind: AttemptKind) -> bool {
        if self.closed || self.in_flight != Some((generation, kind)) {
            return false;
        }
        self.in_flight = None;
        true
    }

    pub(crate) fn is_current(&self, generation: u64) -> bool {
        !self.closed && self.generation == generation
    }

    pub(crate) fn fence(&mut self) {
        if self.closed {
            return;
        }
        self.generation = self.generation.saturating_add(1);
        self.in_flight = None;
    }

    pub(crate) fn close(&mut self) {
        if self.closed {
            return;
        }
        self.fence();
        self.closed = true;
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn in_flight(&self) -> Option<AttemptKind> {
        self.in_flight.map(|(_, kind)| kind)
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.closed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum BootstrapPhase {
    ProbingExisting,
    StartingLocal,
    Ready,
    Recovery,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DiagnosticKind {
    ProbeUnavailable,
    HostIncompatible,
    SidecarSpawn,
    SidecarExited,
    SidecarIncompatible,
    StartupTimeout,
    UnsupportedPlatform,
    HostUnavailable,
    BrowserLaunch,
    Configuration,
    Window,
    Tray,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BootstrapDiagnostic {
    pub(crate) kind: DiagnosticKind,
    pub(crate) message: String,
}

impl BootstrapDiagnostic {
    pub(crate) fn new(kind: DiagnosticKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BootstrapSnapshot {
    pub(crate) phase: BootstrapPhase,
    pub(crate) endpoint: String,
    pub(crate) app_url: String,
    pub(crate) diagnostic: Option<BootstrapDiagnostic>,
    pub(crate) can_open_browser: bool,
    pub(crate) generation: u64,
    pub(crate) in_flight: bool,
    pub(crate) closed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_fence_allows_only_one_flight() {
        let mut fence = GenerationFence::default();
        let generation = fence
            .begin(AttemptKind::StartLocal)
            .expect("first operation should start");
        assert_eq!(fence.in_flight(), Some(AttemptKind::StartLocal));
        assert!(fence.begin(AttemptKind::ProbeExisting).is_none());
        assert!(!fence.finish(generation, AttemptKind::ProbeExisting));
        assert!(fence.finish(generation, AttemptKind::StartLocal));
        assert_eq!(fence.in_flight(), None);
    }

    #[test]
    fn stale_completion_and_fenced_completion_are_rejected() {
        let mut fence = GenerationFence::default();
        let generation = fence
            .begin(AttemptKind::ProbeExisting)
            .expect("operation should start");
        fence.fence();
        assert!(!fence.finish(generation, AttemptKind::ProbeExisting));
        assert!(!fence.is_current(generation));

        let next = fence
            .begin(AttemptKind::StartLocal)
            .expect("new operation should start after a route fence");
        fence.close();
        assert!(!fence.finish(next, AttemptKind::StartLocal));
        assert!(fence.is_closed());
    }

    #[test]
    fn closing_twice_is_a_stable_state_transition() {
        let mut fence = GenerationFence::default();
        fence.close();
        let generation = fence.generation();
        fence.close();
        fence.fence();
        assert_eq!(fence.generation(), generation);
        assert!(fence.is_closed());
        assert!(fence.in_flight().is_none());
    }
}
