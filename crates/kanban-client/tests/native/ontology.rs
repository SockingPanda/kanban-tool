use crate::common::{Host, input, task_request};
use kanban_protocol::{self as dto, cli_labels::CliLabelAtomIndexQueryOutput};
use serde_json::json;

#[tokio::test]
async fn native_ontology_preserves_value_shell_and_typed_business_results() {
    let host = Host::start().await;
    let client = &host.client;
    let task = client
        .create_task("default", task_request("t_ontology_client"))
        .await
        .unwrap();
    let label = client
        .create_board_label("default", &input(json!({"name":"native-ontology"})))
        .await
        .unwrap();
    let semantics = client
        .upsert_label_semantics(
            "default",
            &label.name,
            json!({
                "label_ref":"ignored", "description":"原生兼容", "applies_when":["native transport"]
            }),
        )
        .await
        .unwrap();
    assert_eq!(semantics["data"]["label_id"], label.id);
    client.rebuild_label_atom_index("default").await.unwrap();
    let indexed = client
        .query_label_atom_index("default", Some("native"), None, 5)
        .await
        .unwrap();
    assert_eq!(indexed.as_object().unwrap().len(), 1);
    let indexed: CliLabelAtomIndexQueryOutput = input(indexed);
    assert!(indexed.data.iter().any(|hit| hit.label_id == label.id));
    let suggestions = client
        .suggest_task_labels(
            &task.id,
            Some("default"),
            json!({"limit":"1", "candidate_limit":null}),
        )
        .await
        .unwrap();
    assert_eq!(suggestions["data"]["task_id"], task.id);
    let proposed = client
        .propose_task_label(
            "default",
            &task.id,
            json!({
                "task_ref":"ignored", "name":"new-native-label", "description":"扁平候选",
                "applies_when":["compatibility"], "actor":{"name":"提案者", "type":"user"}
            }),
        )
        .await
        .unwrap();
    assert_eq!(proposed["data"]["proposal"]["name"], "new-native-label");
    let proposal_id = proposed["data"]["proposal"]["id"].as_str().unwrap();
    let decided = client
        .decide_label_proposal(
            proposal_id,
            false,
            json!({
                "reason":"原生回归", "ontology_actor":{"name":"决定者", "actor_type":"user"}
            }),
        )
        .await
        .unwrap();
    assert_eq!(decided["data"]["status"], "rejected");

    let observed = client
        .record_label_ontology_observation(
            "default",
            &task.id,
            json!({
                "task_ref":"ignored", "ontology_actor":{"name":"记录者", "actor_type":"user"},
                "signals":[{
                    "kind":"vocabulary_gap", "target_label_ref":label.name,
                    "related_labels":[], "proposed_action":"observe", "candidate_atom":null,
                    "proposed_label_name":"candidate-native", "proposal":{},
                    "agent_selected":false, "suggest_state":"candidate", "suggest_score":0.3,
                    "suggest_rank":1, "final_selected":false, "rationale":"兼容证据",
                    "confidence":0.7, "signal_key":"native-ontology-signal"
                }]
            }),
        )
        .await
        .unwrap();
    assert_eq!(observed["data"]["task_id"], task.id);
    let signal_id = observed["data"]["signals"][0]["id"].as_str().unwrap();
    for query in [
        json!({"include_all":true,"kind":null,"status":null}),
        json!({"include_all":"true","kind":"vocabulary_gap,false_negative","limit":"1"}),
        json!({"include_all":true,"kind":["vocabulary_gap",42],"limit":1}),
    ] {
        let signals = client
            .list_label_ontology_signals("default", query)
            .await
            .unwrap();
        assert_eq!(signals["data"][0]["id"], signal_id);
        assert_eq!(signals["meta"]["include_all"], true);
    }
    client
        .create_label_ontology_action(
            "default",
            json!({
                "ontology_actor":{"name":"确认者","type":"user"}, "action_type":"confirm",
                "signal_ids":[signal_id], "reason":"确认兼容"
            }),
        )
        .await
        .unwrap();
    let review: dto::ReviewLabelOntologyResponse = input(
        client
            .review_label_ontology(
                "default",
                json!({
                    "group_by":"label", "include_all":"true", "limit":"1"
                }),
            )
            .await
            .unwrap(),
    );
    assert!(review.meta.include_all);
    assert_eq!(review.meta.limit, 1);
    assert_eq!(review.data.len(), 1);
    assert_eq!(review.data[0].confirmed_count, 1);
    host.finish().await;
}
