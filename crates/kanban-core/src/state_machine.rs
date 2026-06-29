use crate::{KanbanError, Result, TaskStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadinessFacts<'a> {
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub scheduled_at: Option<i64>,
    pub dependencies_done: bool,
}

impl ReadinessFacts<'_> {
    fn spec_is_incomplete(self) -> bool {
        self.title.trim().is_empty()
            || self
                .description
                .is_none_or(|description| description.trim().is_empty())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryDecision {
    pub retry_count: i64,
    pub status: TaskStatus,
    pub max_retries_reached: bool,
}

pub fn initial_status(
    explicit: Option<TaskStatus>,
    facts: ReadinessFacts<'_>,
    now: i64,
) -> Result<TaskStatus> {
    if let Some(status) = explicit {
        if !status.can_be_created() {
            return Err(KanbanError::InvalidInput(
                "initial status must be triage/todo/scheduled/ready".into(),
            ));
        }
        match status {
            TaskStatus::Scheduled if facts.scheduled_at.is_none() => {
                return Err(KanbanError::InvalidInput(
                    "scheduled initial status requires scheduled_at".into(),
                ));
            }
            TaskStatus::Ready if facts.spec_is_incomplete() => {
                return Err(KanbanError::InvalidInput(
                    "ready requires description".into(),
                ));
            }
            TaskStatus::Ready if facts.scheduled_at.is_some_and(|scheduled| scheduled > now) => {
                return Err(KanbanError::InvalidInput(
                    "ready requires scheduled_at to be due".into(),
                ));
            }
            _ => return Ok(status),
        }
    }
    Ok(recompute_ready_status(facts, now))
}

pub fn recompute_ready_status(facts: ReadinessFacts<'_>, now: i64) -> TaskStatus {
    if facts.spec_is_incomplete() {
        return TaskStatus::Triage;
    }
    if facts.scheduled_at.is_some_and(|scheduled| scheduled > now) {
        return TaskStatus::Scheduled;
    }
    if !facts.dependencies_done {
        return TaskStatus::Todo;
    }
    TaskStatus::Ready
}

pub const fn is_active_recomputable_status(status: TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Triage | TaskStatus::Todo | TaskStatus::Scheduled | TaskStatus::Ready
    )
}

pub const fn can_promote_from(status: TaskStatus) -> bool {
    matches!(status, TaskStatus::Todo | TaskStatus::Scheduled)
}

pub const fn is_claimable_task(status: TaskStatus, has_claim_token: bool) -> bool {
    status.is_claimable() && !has_claim_token
}

pub const fn can_complete_from(status: TaskStatus) -> bool {
    matches!(status, TaskStatus::Running | TaskStatus::Review)
}

pub const fn can_reopen_from(status: TaskStatus) -> bool {
    matches!(status, TaskStatus::Done)
}

pub const fn can_finish_to(current: TaskStatus, target: TaskStatus) -> bool {
    matches!(current, TaskStatus::Running)
        || matches!((current, target), (TaskStatus::Review, TaskStatus::Done))
}

pub const fn completed_at_for_finish(
    target: TaskStatus,
    now: i64,
    existing_completed_at: Option<i64>,
) -> Option<i64> {
    if matches!(target, TaskStatus::Done) {
        Some(now)
    } else {
        existing_completed_at
    }
}

pub const fn running_claim_is_present(
    status: TaskStatus,
    has_claim_token: bool,
    has_current_run: bool,
) -> bool {
    matches!(status, TaskStatus::Running) && has_claim_token && has_current_run
}

pub const fn retry_decision(
    retry_count: i64,
    max_retries: Option<i64>,
    retry_status: TaskStatus,
) -> RetryDecision {
    let next_retry_count = retry_count + 1;
    let max_retries_reached = match max_retries {
        Some(max_retries) => next_retry_count >= max_retries,
        None => false,
    };
    RetryDecision {
        retry_count: next_retry_count,
        status: if max_retries_reached {
            TaskStatus::Blocked
        } else {
            retry_status
        },
        max_retries_reached,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const PROPTEST_CASES: u32 = 64;

    fn task_status_strategy() -> impl Strategy<Value = TaskStatus> {
        prop_oneof![
            Just(TaskStatus::Triage),
            Just(TaskStatus::Todo),
            Just(TaskStatus::Scheduled),
            Just(TaskStatus::Ready),
            Just(TaskStatus::Running),
            Just(TaskStatus::Blocked),
            Just(TaskStatus::Review),
            Just(TaskStatus::Done),
            Just(TaskStatus::Archived),
        ]
    }

    fn ready_facts() -> ReadinessFacts<'static> {
        ReadinessFacts {
            title: "ship",
            description: Some("ready spec"),
            scheduled_at: None,
            dependencies_done: true,
        }
    }

    #[test]
    fn initial_status_rejects_invalid_explicit_statuses() {
        let err = initial_status(Some(TaskStatus::Running), ready_facts(), 10).unwrap_err();
        assert!(matches!(err, KanbanError::InvalidInput(_)));
    }

    #[test]
    fn initial_status_validates_explicit_ready_and_scheduled_guards() {
        let missing_spec = ReadinessFacts {
            description: None,
            ..ready_facts()
        };
        assert!(initial_status(Some(TaskStatus::Ready), missing_spec, 10).is_err());

        let future = ReadinessFacts {
            scheduled_at: Some(20),
            ..ready_facts()
        };
        assert!(initial_status(Some(TaskStatus::Ready), future, 10).is_err());
        assert!(initial_status(Some(TaskStatus::Scheduled), ready_facts(), 10).is_err());
    }

    #[test]
    fn recompute_ready_status_orders_spec_schedule_dependencies() {
        assert_eq!(
            recompute_ready_status(
                ReadinessFacts {
                    description: None,
                    scheduled_at: Some(20),
                    dependencies_done: false,
                    ..ready_facts()
                },
                10
            ),
            TaskStatus::Triage
        );
        assert_eq!(
            recompute_ready_status(
                ReadinessFacts {
                    scheduled_at: Some(20),
                    dependencies_done: false,
                    ..ready_facts()
                },
                10
            ),
            TaskStatus::Scheduled
        );
        assert_eq!(
            recompute_ready_status(
                ReadinessFacts {
                    dependencies_done: false,
                    ..ready_facts()
                },
                10
            ),
            TaskStatus::Todo
        );
        assert_eq!(recompute_ready_status(ready_facts(), 10), TaskStatus::Ready);
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: PROPTEST_CASES, .. ProptestConfig::default() })]

        #[test]
        fn recompute_ready_status_preserves_priority_order(
            title_present in any::<bool>(),
            description_present in any::<bool>(),
            schedule_delta in -8_i64..=8,
            dependencies_done in any::<bool>(),
            now in -1_000_i64..=1_000,
        ) {
            let facts = ReadinessFacts {
                title: if title_present { "ship" } else { "   " },
                description: description_present.then_some("ready spec"),
                scheduled_at: Some(now + schedule_delta),
                dependencies_done,
            };

            let expected = if !title_present || !description_present {
                TaskStatus::Triage
            } else if schedule_delta > 0 {
                TaskStatus::Scheduled
            } else if !dependencies_done {
                TaskStatus::Todo
            } else {
                TaskStatus::Ready
            };

            prop_assert_eq!(recompute_ready_status(facts, now), expected);
        }

        #[test]
        fn initial_status_explicit_guard_invariants(
            explicit in task_status_strategy(),
            title_present in any::<bool>(),
            description_present in any::<bool>(),
            scheduled in prop::option::of(-8_i64..=8),
            dependencies_done in any::<bool>(),
            now in -1_000_i64..=1_000,
        ) {
            let facts = ReadinessFacts {
                title: if title_present { "ship" } else { "" },
                description: description_present.then_some("ready spec"),
                scheduled_at: scheduled.map(|delta| now + delta),
                dependencies_done,
            };

            let result = initial_status(Some(explicit), facts, now);
            let ready_guard_allows = title_present
                && description_present
                && scheduled.is_none_or(|delta| delta <= 0);
            let should_accept = match explicit {
                TaskStatus::Triage | TaskStatus::Todo => true,
                TaskStatus::Scheduled => scheduled.is_some(),
                TaskStatus::Ready => ready_guard_allows,
                TaskStatus::Running
                | TaskStatus::Blocked
                | TaskStatus::Review
                | TaskStatus::Done
                | TaskStatus::Archived => false,
            };

            if should_accept {
                prop_assert_eq!(result.unwrap(), explicit);
            } else {
                prop_assert!(matches!(result, Err(KanbanError::InvalidInput(_))));
            }
        }

        #[test]
        fn transition_helper_semantics_are_stable(
            current in task_status_strategy(),
            target in task_status_strategy(),
            has_claim_token in any::<bool>(),
            has_current_run in any::<bool>(),
            existing_completed_at in prop::option::of(-1_000_i64..=1_000),
            now in -1_000_i64..=1_000,
        ) {
            prop_assert_eq!(
                is_claimable_task(current, has_claim_token),
                matches!(current, TaskStatus::Ready) && !has_claim_token
            );
            prop_assert_eq!(
                can_complete_from(current),
                matches!(current, TaskStatus::Running | TaskStatus::Review)
            );
            prop_assert_eq!(can_reopen_from(current), matches!(current, TaskStatus::Done));
            prop_assert_eq!(
                can_finish_to(current, target),
                matches!(current, TaskStatus::Running)
                    || matches!((current, target), (TaskStatus::Review, TaskStatus::Done))
            );
            prop_assert_eq!(
                running_claim_is_present(current, has_claim_token, has_current_run),
                matches!(current, TaskStatus::Running) && has_claim_token && has_current_run
            );
            prop_assert_eq!(
                completed_at_for_finish(target, now, existing_completed_at),
                if matches!(target, TaskStatus::Done) { Some(now) } else { existing_completed_at }
            );
        }
    }

    #[test]
    fn exposes_transition_guard_predicates() {
        assert!(is_active_recomputable_status(TaskStatus::Ready));
        assert!(!is_active_recomputable_status(TaskStatus::Running));
        assert!(can_promote_from(TaskStatus::Todo));
        assert!(!can_promote_from(TaskStatus::Review));
        assert!(is_claimable_task(TaskStatus::Ready, false));
        assert!(!is_claimable_task(TaskStatus::Ready, true));
        assert!(can_complete_from(TaskStatus::Review));
        assert!(can_reopen_from(TaskStatus::Done));
        assert!(!can_reopen_from(TaskStatus::Review));
        assert!(can_finish_to(TaskStatus::Review, TaskStatus::Done));
        assert!(!can_finish_to(TaskStatus::Review, TaskStatus::Blocked));
    }

    #[test]
    fn retry_decision_blocks_when_max_retries_is_reached() {
        assert_eq!(
            retry_decision(0, Some(2), TaskStatus::Ready),
            RetryDecision {
                retry_count: 1,
                status: TaskStatus::Ready,
                max_retries_reached: false,
            }
        );
        assert_eq!(
            retry_decision(1, Some(2), TaskStatus::Ready),
            RetryDecision {
                retry_count: 2,
                status: TaskStatus::Blocked,
                max_retries_reached: true,
            }
        );
    }
}
