pub mod clock;
pub mod domain;
pub mod error;
pub mod i18n;
pub mod id;
pub mod state_machine;

pub use clock::{Clock, SystemClock};
pub use domain::{Board, BoardColumn, TaskStatus};
pub use error::{KanbanError, Result};
pub use i18n::{Locale, current_locale, set_current_locale};
pub use id::{new_board_id, new_event_id, new_label_id, new_run_id, new_task_id, new_typed_id};
pub use state_machine::{
    ReadinessFacts, RetryDecision, can_complete_from, can_finish_to, can_promote_from,
    can_reopen_from, completed_at_for_finish, initial_status, is_active_recomputable_status,
    is_claimable_task, recompute_ready_status, retry_decision, running_claim_is_present,
};
