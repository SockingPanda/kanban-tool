//! 既有 Value API 的字段兼容；返回值始终进入具名 RPC 的 typed codec。

use kanban_protocol::{
    LabelOntologyReviewQuery, LabelOntologySignalQuery, LabelProposalDecisionRequest,
    ProposeTaskLabelRequest, rpc::dto::TaskLabelSuggestionQuery,
};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::ClientError;

pub(super) fn from_value<T: DeserializeOwned>(value: Value) -> Result<T, ClientError> {
    serde_json::from_value(value).map_err(|error| ClientError::InvalidInput(error.to_string()))
}

// selector 由方法参数固定；旧调用者附带的同名 body 字段不覆盖 RPC path。
pub(super) fn input_without_selector<T: DeserializeOwned>(
    mut value: Value,
    selector: &str,
) -> Result<T, ClientError> {
    if let Some(object) = value.as_object_mut() {
        object.remove(selector);
    }
    from_value(value)
}

pub(super) fn suggestion_query(
    board: Option<&str>,
    options: Value,
) -> Result<TaskLabelSuggestionQuery, ClientError> {
    let mut query = Map::new();
    for key in [
        "limit",
        "candidate_limit",
        "atom_limit",
        "max_selected_labels",
        "min_score",
    ] {
        let value = match options.get(key) {
            Some(Value::String(value)) => value
                .parse()
                .map_err(|_| ClientError::InvalidInput(format!("{key} 必须是数值")))?,
            Some(Value::Number(value)) => value.clone(),
            _ => continue,
        };
        query.insert(key.to_owned(), Value::Number(value));
    }
    query.insert(
        "board".to_owned(),
        board.map_or(Value::Null, |board| Value::String(board.to_owned())),
    );
    from_value(Value::Object(query))
}

// 旧查询跳过 null/object，数组只保留字符串并用逗号连接。
fn query_scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Array(values) => Some(
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(","),
        ),
        _ => None,
    }
}

pub(super) fn signals_query(value: Value) -> Result<LabelOntologySignalQuery, ClientError> {
    from_value(query_fields(value)?)
}

pub(super) fn review_query(mut value: Value) -> Result<LabelOntologyReviewQuery, ClientError> {
    if let Some(object) = value.as_object_mut()
        && !object.get("group_by").is_some_and(Value::is_string)
    {
        object.remove("group_by");
    }
    from_value(query_fields(value)?)
}

fn query_fields(value: Value) -> Result<Value, ClientError> {
    let mut query = Map::new();
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            let Some(value) = query_scalar(value) else {
                continue;
            };
            let value = match key.as_str() {
                "status" | "kind" => Value::Array(
                    value
                        .split(',')
                        .map(|value| Value::String(value.to_owned()))
                        .collect(),
                ),
                "include_all" => Value::Bool(value.parse().map_err(|_| {
                    ClientError::InvalidInput("include_all 必须是 true 或 false".to_owned())
                })?),
                "limit" => Value::Number(
                    value
                        .parse::<usize>()
                        .map_err(|_| ClientError::InvalidInput("limit 必须是非负整数".to_owned()))?
                        .into(),
                ),
                _ => Value::String(value),
            };
            query.insert(key.clone(), value);
        }
    }
    Ok(Value::Object(query))
}

fn normalize_actor_type(actor: &mut Value) {
    if let Some(actor) = actor.as_object_mut()
        && let Some(actor_type) = actor.remove("actor_type")
    {
        actor.insert("type".to_owned(), actor_type);
    }
}

fn normalize_actors(body: &mut Map<String, Value>) {
    for key in ["actor", "ontology_actor"] {
        if let Some(actor) = body.get_mut(key) {
            normalize_actor_type(actor);
        }
    }
    if !body.contains_key("actor")
        && let Some(Value::Object(actor)) = body.get("ontology_actor")
    {
        body.insert("actor".to_owned(), Value::Object(actor.clone()));
    }
}

fn actor_name(body: &mut Map<String, Value>) {
    normalize_actors(body);
    if let Some(Value::Object(actor)) = body.get("actor") {
        let name = actor
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("user")
            .to_owned();
        body.insert("actor".to_owned(), Value::String(name));
    }
}

pub(super) fn proposal_input(value: Value) -> Result<ProposeTaskLabelRequest, ClientError> {
    let mut body: Map<String, Value> = from_value(value)?;
    body.remove("task_ref");
    actor_name(&mut body);
    let mut candidate = match body.remove("proposal") {
        Some(Value::Object(candidate)) => candidate,
        None | Some(Value::Null) => Map::new(),
        Some(_) => {
            return Err(ClientError::InvalidInput("proposal 必须是对象".to_owned()));
        }
    };
    for key in [
        "name",
        "description",
        "applies_when",
        "excludes_when",
        "positive_examples",
        "negative_examples",
    ] {
        if let Some(value) = body.remove(key) {
            // 旧嵌套 proposal 的字段优先于同名顶层字段。
            candidate.entry(key.to_owned()).or_insert(value);
        }
    }
    if !candidate.is_empty() {
        body.insert("proposal".to_owned(), Value::Object(candidate));
    }
    from_value(Value::Object(body))
}

pub(super) fn decision_input(value: Value) -> Result<LabelProposalDecisionRequest, ClientError> {
    let mut body: Map<String, Value> = from_value(value)?;
    body.remove("proposal_id");
    body.remove("accept");
    actor_name(&mut body);
    from_value(Value::Object(body))
}

pub(super) fn actor_input<T: DeserializeOwned>(
    value: Value,
    selector: Option<&str>,
) -> Result<T, ClientError> {
    let mut body: Map<String, Value> = from_value(value)?;
    if let Some(selector) = selector {
        body.remove(selector);
    }
    normalize_actors(&mut body);
    body.remove("ontology_actor");
    from_value(Value::Object(body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn body_selectors_are_consumed_and_other_unknown_fields_are_rejected() {
        let semantics: kanban_protocol::UpsertLabelSemanticsRequest = input_without_selector(
            json!({"label_ref":"ignored", "description":"语义"}),
            "label_ref",
        )
        .unwrap();
        assert_eq!(semantics.description.as_deref(), Some("语义"));
        assert!(proposal_input(json!({"task_ref":"ignored"})).is_ok());
        assert!(proposal_input(json!({"unrecognized":true})).is_err());
        assert!(proposal_input(json!([])).is_err());
    }

    #[test]
    fn signal_queries_preserve_null_scalar_csv_and_array_inputs() {
        let empty = signals_query(json!({"kind":null,"status":null,"limit":null})).unwrap();
        assert!(empty.kind.is_empty());
        assert!(empty.status.is_empty());
        assert_eq!(empty.limit, 100);
        let query = signals_query(json!({
            "kind":["false_negative", 7, "false_positive,boundary_issue"],
            "status":"proposed,confirmed", "include_all":"true", "limit":"25",
            "task_ref":42,"target_label_ref":{"ignored":true},"proposed_label_name":false
        }))
        .unwrap();
        assert_eq!(
            query.kind,
            ["false_negative", "false_positive", "boundary_issue"]
        );
        assert_eq!(query.status, ["proposed", "confirmed"]);
        assert!(query.include_all);
        assert_eq!(query.limit, 25);
        assert_eq!(query.task_ref.as_deref(), Some("42"));
        assert_eq!(query.target_label_ref, None);
        assert_eq!(query.proposed_label_name.as_deref(), Some("false"));
        assert!(signals_query(json!({"include_all":"yes"})).is_err());
        assert!(signals_query(json!({"limit":-1})).is_err());
    }

    #[test]
    fn suggestion_queries_skip_absent_values_and_accept_numeric_strings() {
        let query = suggestion_query(
            Some("default"),
            json!({
                "limit":"7", "min_score":"0.25", "candidate_limit":null,
                "atom_limit":false,"max_selected_labels":[],"unused":"ignored"
            }),
        )
        .unwrap();
        assert_eq!(query.limit, 7);
        assert_eq!(query.min_score, 0.25);
        assert_eq!(query.candidate_limit, 32);
        assert_eq!(query.atom_limit, 80);
        assert_eq!(query.max_selected_labels, 4);
        assert_eq!(query.board.as_deref(), Some("default"));
    }

    #[test]
    fn review_queries_preserve_all_explicit_filters() {
        let query =
            review_query(json!({"group_by":null,"include_all":"true","limit":"1"})).unwrap();
        assert_eq!(
            query.group_by,
            kanban_protocol::LabelOntologyReviewGroupByWire::Label
        );
        assert!(query.include_all);
        assert_eq!(query.limit, 1);
    }

    #[test]
    fn proposals_accept_flat_and_nested_candidates_and_actor_objects() {
        let request = proposal_input(json!({
            "task_ref":"ignored", "name":"flat", "description":"保留顶层描述",
            "proposal":{"name":"nested", "applies_when":["条件"]},
            "actor":{"name":"名字", "actor_type":"user"}
        }))
        .unwrap();
        let candidate = request.proposal.unwrap();
        assert_eq!(candidate.name, "nested");
        assert_eq!(candidate.description.as_deref(), Some("保留顶层描述"));
        assert_eq!(candidate.applies_when, ["条件"]);
        assert_eq!(request.actor.as_deref(), Some("名字"));
        let request = decision_input(json!({
            "proposal_id":"ignored","accept":true,
            "ontology_actor":{"name":"决定者","actor_type":"user"}
        }))
        .unwrap();
        assert_eq!(request.actor.as_deref(), Some("决定者"));
        assert_eq!(request.ontology_actor.unwrap().actor_type, "user");
    }

    #[test]
    fn observations_accept_the_existing_actor_alias() {
        let request: kanban_protocol::RecordLabelOntologyObservationRequest = actor_input(
            json!({"task_ref":"ignored", "ontology_actor":{"name":"记录者","actor_type":"user"},"signals":[]}),
            Some("task_ref"),
        ).unwrap();
        assert_eq!(request.actor.name, "记录者");
        assert_eq!(request.actor.actor_type, "user");
    }
}
