use kanban_service::{
    SignalCreateRecord as ApplicationCreateSignal, SignalLedger,
    SignalListOptions as ApplicationSignalListOptions, SignalRecord as ApplicationSignal,
    SignalRecordResult as ApplicationSignalResult, SignalReviewRecord as ApplicationReviewSignals,
};
use kanban_core::Result;
use kanban_store_turso::{
    CreateSignalInput as StoreCreateSignal, ReviewSignalsInput as StoreReviewSignals,
    SignalLifecycleInput as StoreSignalLifecycle, SignalListOptions as StoreSignalListOptions,
};

use crate::adapter::{
    TursoApplicationStore, application_signal, application_signal_result, store_error,
};

impl SignalLedger for TursoApplicationStore {
    async fn record_signal(
        &self,
        input: ApplicationCreateSignal,
    ) -> Result<ApplicationSignalResult> {
        self.store
            .record_signal(StoreCreateSignal {
                id: input.id,
                observation_id: input.observation_id,
                event_id: input.event_id,
                board: input.board,
                kind: input.kind,
                title: input.title,
                summary: input.summary,
                severity: input.severity,
                task_ref: input.task_ref,
                task_id: input.task_id,
                run_id: input.run_id,
                comment_id: input.comment_id,
                actor: input.actor,
                agent_type: input.agent_type,
                dedupe_key: input.dedupe_key,
                source: input.source,
                evidence_json: input.evidence_json,
                comment_body: input.comment_body,
                created_at: input.created_at,
            })
            .await
            .map_err(store_error)
            .and_then(application_signal_result)
    }

    async fn list_signals(
        &self,
        board: &str,
        options: ApplicationSignalListOptions,
    ) -> Result<Vec<ApplicationSignal>> {
        self.store
            .list_signals(
                board,
                StoreSignalListOptions {
                    statuses: options
                        .statuses
                        .into_iter()
                        .map(|status| status.as_str().to_owned())
                        .collect(),
                    kinds: options.kinds,
                    task_ref: options.task_ref,
                    include_all: options.include_all,
                    limit: options.limit,
                },
            )
            .await
            .map_err(store_error)?
            .into_iter()
            .map(application_signal)
            .collect()
    }

    async fn get_signal(&self, signal_id: &str) -> Result<ApplicationSignal> {
        self.store
            .get_signal(signal_id)
            .await
            .map_err(store_error)
            .and_then(application_signal)
    }

    async fn review_signals(
        &self,
        input: ApplicationReviewSignals,
    ) -> Result<Vec<ApplicationSignal>> {
        self.store
            .review_signals(StoreReviewSignals {
                board: input.board,
                signal_ids: input.signal_ids,
                lifecycle: match input.lifecycle {
                    kanban_service::SignalLifecycle::Confirm => StoreSignalLifecycle::Confirm,
                    kanban_service::SignalLifecycle::Reject => StoreSignalLifecycle::Reject,
                    kanban_service::SignalLifecycle::Resolve => StoreSignalLifecycle::Resolve,
                    kanban_service::SignalLifecycle::Supersede => {
                        StoreSignalLifecycle::Supersede
                    }
                },
                replacement_signal_id: input.replacement_signal_id,
                actor: input.actor,
                reason: input.reason,
                event_ids: input.event_ids,
                now: input.now,
            })
            .await
            .map_err(store_error)?
            .into_iter()
            .map(application_signal)
            .collect()
    }
}
