use kanban_core::{KanbanError, Result};

pub(crate) fn infer_comment_author_type(kind: &str) -> &'static str {
    match kind {
        "worker" => "agent",
        "system" => "system",
        _ => "human",
    }
}

pub(crate) fn normalize_comment_author_type<'a>(
    author_type: Option<&'a str>,
    kind: &str,
) -> Result<&'a str> {
    match author_type.map(str::trim) {
        Some("human") => Ok("human"),
        Some("agent") => Ok("agent"),
        Some("system") => Ok("system"),
        Some(_) => Err(KanbanError::InvalidInput(
            "invalid comment author_type".into(),
        )),
        None => Ok(infer_comment_author_type(kind)),
    }
}

pub(crate) fn normalize_comment_agent_type<'a>(
    agent_type: Option<&'a str>,
    author_type: &str,
) -> Result<Option<&'a str>> {
    let Some(agent_type) = agent_type.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if author_type != "agent" {
        return Err(KanbanError::InvalidInput(
            "comment agent_type is only allowed when author_type is agent".into(),
        ));
    }
    Ok(Some(agent_type))
}
