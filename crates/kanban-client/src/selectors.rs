use kanban_contract::{ApiErrorCode, ListTasksQuery};

use crate::{KanbanClient, error::ClientError};

impl KanbanClient {
    pub fn resolve_task_id(&self, board: &str, selector: &str) -> Result<String, ClientError> {
        let selector = selector.trim();
        if selector.starts_with("t_") && selector.len() > 2 {
            return Ok(selector.to_owned());
        }
        if !is_board_local_task_selector(selector) {
            return Err(ClientError::InvalidInput(
                "task selector must be a global t_... id, board#seq, #seq, or numeric seq"
                    .to_owned(),
            ));
        }
        let response = self.list_tasks(
            board,
            &ListTasksQuery {
                q: Some(selector.to_owned()),
                include_archived: true,
                limit: 2,
                ..ListTasksQuery::default()
            },
        )?;
        match response.data.as_slice() {
            [task] => Ok(task.id.clone()),
            [] => Err(ClientError::Api {
                status: 404,
                code: ApiErrorCode::NotFound,
                message: format!("task not found: {selector}"),
            }),
            _ => Err(ClientError::InvalidResponse(format!(
                "task selector is ambiguous: {selector}"
            ))),
        }
    }

    pub(crate) fn resolve_step_id(
        &self,
        task_id: &str,
        selector: &str,
    ) -> Result<String, ClientError> {
        let selector = selector.trim();
        if selector.starts_with("step_") && selector.len() > 5 {
            return Ok(selector.to_owned());
        }
        let index = selector
            .strip_prefix('S')
            .or_else(|| selector.strip_prefix('s'))
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|index| *index > 0)
            .ok_or_else(|| {
                ClientError::InvalidInput(
                    "step selector must be a global step_... id or S<n>".to_owned(),
                )
            })?;
        let steps = self.list_steps(task_id)?;
        steps
            .steps
            .get(index - 1)
            .map(|step| step.id.clone())
            .ok_or_else(|| ClientError::Api {
                status: 404,
                code: ApiErrorCode::NotFound,
                message: format!("step not found: {selector}"),
            })
    }
}

fn is_board_local_task_selector(selector: &str) -> bool {
    let numeric = |value: &str| {
        !value.is_empty() && value.chars().all(|character| character.is_ascii_digit())
    };
    if let Some(seq) = selector.strip_prefix('#') {
        return numeric(seq);
    }
    if numeric(selector) {
        return true;
    }
    selector
        .split_once('#')
        .is_some_and(|(board, seq)| !board.is_empty() && numeric(seq))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_selector_classification_is_narrow_and_deterministic() {
        for selector in ["#1", "1", "default#1", "b_default#42"] {
            assert!(is_board_local_task_selector(selector), "{selector}");
        }
        for selector in ["", "#", "default", "default#x", "default#1#2"] {
            assert!(!is_board_local_task_selector(selector), "{selector}");
        }
    }
}
