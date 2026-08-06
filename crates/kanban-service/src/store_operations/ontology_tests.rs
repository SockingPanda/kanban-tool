#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread::JoinHandle;

    use crate::domain::LabelAtomRecord;
    use crate::store_operations::{
        CreateTaskInput, LabelSuggestionOptions, OntologyActorInput, OntologyApplyAtomInput,
        OntologyRevertInput, UpsertLabelSemanticsInput,
    };
    use crate::test_support::*;
    use crate::vector::{VectorConfig, VectorEmbeddingInput, stable_id};

    async fn label(store: &TursoStore, id: &str, name: &str) {
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO labels(id,board_id,name,created_at,updated_at) VALUES (:id,'b_default',:name,1,1)",
                turso::named_params! { ":id": id, ":name": name },
            )
            .await
            .expect("label");
    }

    fn user() -> OntologyActorInput {
        OntologyActorInput {
            name: "tester".to_owned(),
            actor_type: "user".to_owned(),
            agent_type: None,
        }
    }

    fn task_input(id: &str, title: &str, description: Option<&str>) -> CreateTaskInput {
        CreateTaskInput {
            id: id.to_owned(),
            idempotency_key: None,
            title: title.to_owned(),
            status: "todo".to_owned(),
            description: description.map(str::to_owned),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".to_owned(),
            labels: Vec::new(),
            depends_on: Vec::new(),
            created_by: "tester".to_owned(),
        }
    }

    fn mock_ollama(embedding: Vec<f32>) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind mock Ollama");
        let address = listener.local_addr().expect("mock Ollama address");
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("Ollama request");
            let request = read_http_request(&mut stream);
            assert!(request.contains("\"model\""));
            let body = serde_json::json!({"embeddings": [embedding]}).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write Ollama response");
        });
        (format!("http://{address}"), handle)
    }

    fn read_http_request(stream: &mut TcpStream) -> String {
        let mut bytes = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let count = stream.read(&mut chunk).expect("read Ollama request");
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..count]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        String::from_utf8(bytes).expect("Ollama request utf8")
    }

    async fn make_vector_ready(
        store: &TursoStore,
        board_id: &str,
        config: &VectorConfig,
        generation: &str,
    ) {
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE projection_jobs SET status='done',lease_owner=NULL,lease_token=NULL,lease_expires_at=NULL WHERE target='vector_label_atoms' AND board_id=:board",
                [(":board", board_id)],
            )
            .await
            .expect("finish vector jobs");
        connection
            .execute(
                "UPDATE projection_state SET lifecycle_status='ready',active_generation=:generation,active_fingerprint=:fingerprint,provider='ollama',provider_fingerprint=:fingerprint,embedding_model=:model,embedding_dimensions=:dimensions,dirty=0,last_error=NULL,updated_at=1 WHERE projection IN ('vector_tasks','vector_label_atoms')",
                turso::named_params! {
                    ":generation": generation,
                    ":fingerprint": config.fingerprint(),
                    ":model": config.model.as_str(),
                    ":dimensions": config.dimensions as i64,
                },
            )
            .await
            .expect("publish vector state");
        connection
            .execute(
                "INSERT INTO label_atom_index_boards(store_name,board_id,dirty,last_rebuild_at,last_error,updated_at) VALUES (:store,:board,0,1,NULL,1) ON CONFLICT(store_name,board_id) DO UPDATE SET dirty=0,last_error=NULL,updated_at=1",
                turso::named_params! {
                    ":store": crate::store_operations::ontology::LABEL_ATOM_INDEX_STORE,
                    ":board": board_id,
                },
            )
            .await
            .expect("publish board vector state");
    }

    async fn seed_atom_vectors(store: &TursoStore, model: &str, atoms: &[LabelAtomRecord]) {
        for atom in atoms {
            let document = store
                .vector_label_atom_document(&atom.id)
                .await
                .expect("atom document")
                .expect("atom document exists");
            let vector = match atom.label_name.as_str() {
                "Backend" => vec![1.0, 0.0, 0.0],
                "Frontend" if atom.polarity == "negative" => vec![1.0, 0.0, 0.0],
                "Frontend" => vec![1.0, 0.0, 0.0],
                _ => vec![0.0, 1.0, 0.0],
            };
            store
                .upsert_vector_document(&document)
                .await
                .expect("vector document");
            store
                .upsert_vector_embedding(&VectorEmbeddingInput {
                    id: stable_id("vec", &[&document.id, model]),
                    board_id: document.board_id.clone(),
                    entity_uri: document.entity_uri.clone(),
                    document_id: document.id.clone(),
                    embedding: vector,
                    dimensions: 3,
                    embedding_model: model.to_owned(),
                    content_hash: document.content_hash.clone(),
                    created_at: document.created_at,
                    updated_at: document.updated_at,
                })
                .await
                .expect("vector embedding");
        }
    }

    #[tokio::test]
    async fn provider_backed_suggestions_use_turso_vectors_and_preserve_evidence() {
        let (_directory, store, _path) = store("ontology-vector-suggestions").await;
        store.initialize().await.expect("initialize");
        label(&store, "l_backend", "Backend").await;
        label(&store, "l_frontend", "Frontend").await;
        let task = store
            .create_task(
                "default",
                task_input("t_vector_suggestion", "Fix server API", Some("handler")),
            )
            .await
            .expect("task");
        store
            .add_task_labels(
                &task.id,
                crate::store_operations::AddTaskLabelsInput {
                    names: vec!["Backend".to_owned()],
                    label_ids: vec!["l_backend".to_owned()],
                    event_ids: vec!["e_vector_suggestion_label".to_owned()],
                    create_missing: false,
                    actor: "tester".to_owned(),
                    now: 2,
                },
            )
            .await
            .expect("applied label");
        for (id, description, excludes) in [
            ("l_backend", "server API handlers", Vec::new()),
            (
                "l_frontend",
                "server API handlers",
                vec!["server only".to_owned()],
            ),
        ] {
            store
                .upsert_label_semantics(
                    "default",
                    id,
                    UpsertLabelSemanticsInput {
                        expected_semantics_hash: None,
                        replace: true,
                        description: Some(description.to_owned()),
                        applies_when: Some(vec!["server API".to_owned()]),
                        excludes_when: Some(excludes),
                        positive_examples: None,
                        negative_examples: None,
                        remove_applies_when: Vec::new(),
                        remove_excludes_when: Vec::new(),
                        remove_positive_examples: Vec::new(),
                        remove_negative_examples: Vec::new(),
                        actor: "tester".to_owned(),
                        reason: Some("seed".to_owned()),
                        source_signal_ids: Vec::new(),
                    },
                )
                .await
                .expect("semantics");
        }
        let atoms = store.list_label_atoms("default").await.expect("atoms");
        let endpoint_embedding = vec![1.0, 0.0, 0.0];
        let (endpoint, server) = mock_ollama(endpoint_embedding);
        let config = VectorConfig {
            provider: "ollama".to_owned(),
            endpoint,
            model: "mock-label-model".to_owned(),
            dimensions: 3,
        };
        store.configure_vector(&config).await.expect("configure");
        seed_atom_vectors(&store, &config.model, &atoms).await;
        make_vector_ready(&store, "b_default", &config, "generation-1").await;
        let index_status = store
            .label_atom_index_status("default")
            .await
            .expect("index status");
        assert!(
            index_status.enabled,
            "healthy provider index should be enabled"
        );

        let result = store
            .suggest_task_labels(
                "default",
                &task.id,
                LabelSuggestionOptions {
                    min_score: 0.01,
                    ..LabelSuggestionOptions::default()
                },
            )
            .await
            .expect("suggestions");
        server.join().expect("mock Ollama thread");

        assert!(
            !result.degraded,
            "provider result should be trusted: {result:?}"
        );
        assert_eq!(result.selected_labels[0].label_name, "Backend");
        assert!(result.selected_labels[0].already_applied);
        assert!(result.coverage > 0.99);
        assert!(result.coverage_cosine > 0.99);
        assert!(result.residual_norm < 0.01);
        let frontend = result
            .candidates
            .iter()
            .find(|candidate| candidate.label_name == "Frontend")
            .expect("negative evidence candidate");
        assert!(!frontend.negative_evidence_atoms.is_empty());
    }

    #[tokio::test]
    async fn provider_outage_returns_degraded_without_writing_canonical_facts() {
        let (_directory, store, _path) = store("ontology-vector-outage").await;
        store.initialize().await.expect("initialize");
        label(&store, "l_outage", "Outage").await;
        let task = store
            .create_task(
                "default",
                task_input("t_vector_outage", "provider outage", None),
            )
            .await
            .expect("task");
        let config = VectorConfig {
            provider: "ollama".to_owned(),
            endpoint: "http://127.0.0.1:1".to_owned(),
            model: "mock-label-model".to_owned(),
            dimensions: 3,
        };
        store.configure_vector(&config).await.expect("configure");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE projection_state SET lifecycle_status='ready',active_generation='generation-1',active_fingerprint=:fingerprint,provider='ollama',provider_fingerprint=:fingerprint,embedding_model=:model,embedding_dimensions=3,dirty=0,last_error=NULL WHERE projection='vector_label_atoms'",
                turso::named_params! {
                    ":fingerprint": config.fingerprint(),
                    ":model": config.model.as_str(),
                },
            )
            .await
            .expect("ready state");
        let jobs_before = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM projection_jobs WHERE target='vector_label_atoms'",
                    (),
                )
                .await
                .expect("jobs"),
        )
        .await
        .expect("jobs row");
        let jobs_before = integer_value(jobs_before.get_value(0).expect("jobs count"), "jobs")
            .expect("jobs count value");

        let result = store
            .suggest_task_labels("default", &task.id, LabelSuggestionOptions::default())
            .await
            .expect("degraded suggestion");
        assert!(result.degraded);
        assert!(result.selected_labels.is_empty());
        assert!(
            result
                .reason_codes
                .contains(&"vector_query_error".to_owned())
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|value| value.contains("连接 Ollama") || value.contains("连接"))
        );

        let jobs_after = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM projection_jobs WHERE target='vector_label_atoms'",
                    (),
                )
                .await
                .expect("jobs"),
        )
        .await
        .expect("jobs row");
        let jobs_after = integer_value(jobs_after.get_value(0).expect("jobs count"), "jobs")
            .expect("jobs count value");
        assert_eq!(
            jobs_before, jobs_after,
            "suggestion must not write projection jobs"
        );
    }

    #[tokio::test]
    async fn delete_semantics_removes_derived_atoms() {
        let (_directory, store, _path) = store("ontology-delete").await;
        store.initialize().await.expect("initialize");
        label(&store, "l_delete", "Delete me").await;
        let semantics = store
            .upsert_label_semantics(
                "default",
                "l_delete",
                UpsertLabelSemanticsInput {
                    expected_semantics_hash: None,
                    replace: true,
                    description: Some("description".to_owned()),
                    applies_when: Some(vec!["applies".to_owned()]),
                    excludes_when: None,
                    positive_examples: None,
                    negative_examples: None,
                    remove_applies_when: Vec::new(),
                    remove_excludes_when: Vec::new(),
                    remove_positive_examples: Vec::new(),
                    remove_negative_examples: Vec::new(),
                    actor: "tester".to_owned(),
                    reason: Some("seed".to_owned()),
                    source_signal_ids: Vec::new(),
                },
            )
            .await
            .expect("upsert semantics");
        assert!(!semantics.atoms.is_empty());
        store
            .delete_label_semantics(
                "default",
                "l_delete",
                &semantics.semantics_hash,
                "remove",
                "tester",
            )
            .await
            .expect("delete semantics");
        assert!(
            store
                .list_label_atoms("default")
                .await
                .expect("atoms")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn apply_and_revert_keep_canonical_hashes_and_atoms_in_sync() {
        let (_directory, store, _path) = store("ontology-apply-revert").await;
        store.initialize().await.expect("initialize");
        label(&store, "l_apply", "Apply me").await;
        let action = store
            .apply_label_ontology_atom(
                "default",
                OntologyApplyAtomInput {
                    actor: user(),
                    signal_ids: Vec::new(),
                    label_ref: "l_apply".to_owned(),
                    polarity: "positive".to_owned(),
                    kind: "applies_when".to_owned(),
                    text: "new behavior".to_owned(),
                    reason: "capture atom".to_owned(),
                },
            )
            .await
            .expect("apply atom");
        assert_eq!(action.action_type, "add_positive_atom");
        assert!(action.canonical_after_hash.is_some());
        assert_eq!(action.validation_status, "pending");
        let applied = store
            .get_label_semantics("default", "l_apply")
            .await
            .expect("applied semantics");
        assert!(
            applied
                .applies_when
                .iter()
                .any(|value| value == "new behavior")
        );
        let explained = store
            .explain_label_atom("default", &action.result_atom_id.clone().expect("atom id"))
            .await
            .expect("explain atom");
        assert!(!explained.provenance_actions.is_empty());

        let reverted = store
            .revert_label_ontology_mutation(
                "default",
                OntologyRevertInput {
                    actor: user(),
                    target_action_id: action.id,
                    expected_current_hash: action.canonical_after_hash,
                    reason: "undo atom".to_owned(),
                },
            )
            .await
            .expect("revert atom");
        assert_eq!(reverted.action_type, "revert_ontology_mutation");
        let restored = store
            .get_label_semantics("default", "l_apply")
            .await
            .expect("restored semantics");
        assert!(
            !restored
                .applies_when
                .iter()
                .any(|value| value == "new behavior")
        );
        assert!(
            store
                .list_label_atoms("default")
                .await
                .expect("restored atoms")
                .iter()
                .all(|atom| atom.text != "new behavior")
        );
    }
}
