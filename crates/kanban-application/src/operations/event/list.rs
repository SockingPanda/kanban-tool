use std::future::Future;

use kanban_core::{Clock, KanbanError, Result};

use crate::{ApplicationService, ApplicationStore};

const MAX_EVENT_LIST_LIMIT: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRecord {
    pub id: i64,
    pub event_id: String,
    pub board_id: String,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub kind: String,
    pub actor: Option<String>,
    pub payload_json: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventListOptions {
    pub task_id: Option<String>,
    pub after: i64,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventListPage {
    pub events: Vec<EventRecord>,
    pub next_after: i64,
}

pub trait EventList: ApplicationStore {
    fn list_events(
        &self,
        board: &str,
        options: EventListOptions,
    ) -> impl Future<Output = Result<EventListPage>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: EventList,
    C: Clock,
{
    pub async fn list_events(
        &self,
        board: &str,
        mut options: EventListOptions,
    ) -> Result<EventListPage> {
        let board = board.trim();
        if board.is_empty() {
            return Err(KanbanError::InvalidInput("board is required".to_owned()));
        }
        if options.after < 0 {
            return Err(KanbanError::InvalidInput(
                "after must be non-negative".to_owned(),
            ));
        }
        options.task_id = options.task_id.map(|task_id| task_id.trim().to_owned());
        if let Some(task_id) = options.task_id.as_deref()
            && (!task_id.starts_with("t_") || task_id.len() <= 2)
        {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        options.limit = options.limit.min(MAX_EVENT_LIST_LIMIT);
        self.store.list_events(board, options).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;

    use kanban_core::{KanbanError, Result};

    use crate::operations::test_support::{FixedClock, StubStore};
    use crate::*;

    impl EventList for StubStore {
        async fn list_events(
            &self,
            board: &str,
            options: EventListOptions,
        ) -> Result<EventListPage> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(board, "default");
            if options.limit == MAX_EVENT_LIST_LIMIT {
                assert_eq!(options.task_id.as_deref(), Some("t_event"));
            }
            if options.limit == 0 {
                return Ok(EventListPage {
                    events: Vec::new(),
                    next_after: options.after,
                });
            }
            Ok(EventListPage {
                events: vec![EventRecord {
                    id: 17,
                    event_id: "e_unknown".into(),
                    board_id: "b_default".into(),
                    task_id: None,
                    run_id: None,
                    kind: "future.event".into(),
                    actor: None,
                    payload_json: r#"["opaque",1]"#.into(),
                    created_at: 99,
                }],
                next_after: 17,
            })
        }
    }

    fn service(calls: Arc<AtomicUsize>) -> ApplicationService<StubStore, FixedClock> {
        ApplicationService::with_clock(StubStore { calls }, FixedClock(100))
    }

    #[tokio::test]
    async fn list_events_canonicalizes_inputs_caps_limit_and_preserves_raw_page() {
        let calls = Arc::new(AtomicUsize::new(0));
        let page = service(calls.clone())
            .list_events(
                " default ",
                EventListOptions {
                    task_id: Some(" t_event ".into()),
                    after: 7,
                    limit: usize::MAX,
                },
            )
            .await
            .unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(page.next_after, 17);
        assert_eq!(page.events[0].kind, "future.event");
        assert_eq!(page.events[0].payload_json, r#"["opaque",1]"#);
        assert_eq!(page.events[0].actor, None);
        assert_eq!(page.events[0].task_id, None);
    }

    #[tokio::test]
    async fn list_events_allows_zero_limit_and_does_not_take_mutation_gate() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = service(calls.clone());
        let _mutation = service.mutation_gate.lock().await;
        let query = tokio::time::timeout(
            Duration::from_millis(100),
            service.list_events(
                "default",
                EventListOptions {
                    task_id: Some("t_event".into()),
                    after: 0,
                    limit: 0,
                },
            ),
        )
        .await
        .expect("queries must not wait for the mutation gate")
        .unwrap();

        assert!(query.events.is_empty());
        assert_eq!(query.next_after, 0);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn list_events_rejects_invalid_inputs_without_calling_store() {
        let calls = Arc::new(AtomicUsize::new(0));

        let error = service(calls.clone())
            .list_events(
                "   ",
                EventListOptions {
                    task_id: None,
                    after: 0,
                    limit: 0,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, KanbanError::InvalidInput(message) if message.contains("board")));

        let error = service(calls.clone())
            .list_events(
                "default",
                EventListOptions {
                    task_id: Some("   ".into()),
                    after: 0,
                    limit: 0,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, KanbanError::InvalidInput(message) if message.contains("global")));

        let error = service(calls.clone())
            .list_events(
                "default",
                EventListOptions {
                    task_id: Some("default#1".into()),
                    after: 0,
                    limit: 0,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, KanbanError::InvalidInput(message) if message.contains("global")));

        let error = service(calls.clone())
            .list_events(
                "default",
                EventListOptions {
                    task_id: None,
                    after: -1,
                    limit: 0,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, KanbanError::InvalidInput(message) if message.contains("after")));

        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn list_events_returns_store_errors_unchanged() {
        #[derive(Clone)]
        struct ErrorStore;

        impl ApplicationStore for ErrorStore {}

        impl EventList for ErrorStore {
            async fn list_events(
                &self,
                _board: &str,
                _options: EventListOptions,
            ) -> Result<EventListPage> {
                Err(KanbanError::FeatureNotAvailable("store error".into()))
            }
        }

        let service = ApplicationService::with_clock(ErrorStore, FixedClock(100));
        let error = service
            .list_events(
                "default",
                EventListOptions {
                    task_id: None,
                    after: 0,
                    limit: 0,
                },
            )
            .await
            .unwrap_err();
        assert!(
            matches!(error, KanbanError::FeatureNotAvailable(message) if message == "store error")
        );
    }
}
