#[cfg(test)]
mod tests {
    use crate::test_support::{create_input, integer_value, store, text_value};

    #[tokio::test]
    async fn vector32_roundtrip_dimension_and_cosine_are_real_turso_capabilities() {
        let (_directory, store, _path) = store("vector-capability").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO retrieval_vectors(id, embedding, dimensions, embedding_model, content_hash, created_at, updated_at) VALUES ('vec_test', vector32('[1.0, 0.0]'), 2, 'test', 'hash', 1, 1)",
                (),
            )
            .await
            .expect("vector32 insert");
        let mut rows = connection
            .query(
                "SELECT typeof(embedding), dimensions, vector_distance_cos(embedding, vector32('[1.0, 0.0]')) FROM retrieval_vectors WHERE id='vec_test'",
                (),
            )
            .await
            .expect("vector query");
        let row = rows
            .next()
            .await
            .expect("vector row")
            .expect("vector result");
        assert_eq!(
            text_value(row.get_value(0).expect("vector type"), "vector type")
                .expect("vector type text"),
            "blob"
        );
        assert_eq!(
            integer_value(row.get_value(1).expect("dimensions"), "dimensions")
                .expect("dimensions integer"),
            2
        );
        let distance = match row.get_value(2).expect("cosine distance") {
            turso::Value::Real(value) => value,
            turso::Value::Integer(value) => value as f64,
            value => panic!("unexpected cosine value: {value:?}"),
        };
        assert!(distance.abs() < 1e-6, "cosine distance was {distance}");
        let mut mismatch_rows = connection
            .query(
                "SELECT vector_distance_cos(embedding, vector32('[1.0]')) FROM retrieval_vectors WHERE id='vec_test'",
                (),
            )
            .await
            .expect("创建维度不匹配查询");
        let mismatch = match mismatch_rows.next().await {
            Err(error) => error,
            Ok(_) => panic!("Turso 必须拒绝维度不匹配的向量"),
        };
        assert!(mismatch.to_string().contains("dimension"));
    }

    #[tokio::test]
    async fn vector_label_query_returns_canonical_label_name() {
        let (_directory, store, _path) = store("vector-label-name").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO labels(id, board_id, name, created_at, updated_at) VALUES ('l_vector', 'b_default', 'Vector label', 1, 1)",
                (),
            )
            .await
            .expect("label insert");
        connection
            .execute(
                "INSERT INTO label_atoms(id, label_id, board_id, polarity, kind, text, ordinal, content_hash, created_at, updated_at) VALUES ('la_vector', 'l_vector', 'b_default', 'positive', 'description', 'vector semantics', 0, 'hash-vector', 1, 1)",
                (),
            )
            .await
            .expect("label atom insert");
        connection
            .execute(
                "INSERT INTO retrieval_documents(id, board_id, source_kind, content, content_hash, created_at, updated_at) VALUES ('doc_vector', 'b_default', 'label_atom', 'vector semantics', 'hash-vector', 1, 1)",
                (),
            )
            .await
            .expect("retrieval document insert");
        connection
            .execute(
                "INSERT INTO retrieval_vectors(id, board_id, document_id, embedding, dimensions, embedding_model, content_hash, created_at, updated_at) VALUES ('vec_vector', 'b_default', 'doc_vector', vector32('[1.0, 0.0]'), 2, 'test-model', 'hash-vector', 1, 1)",
                (),
            )
            .await
            .expect("retrieval vector insert");
        let hits = store
            .query_vector_label_atoms(Some("b_default"), &[1.0, 0.0], "test-model", None, 5, false)
            .await
            .expect("label atom vector query");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].label_name, "Vector label");
    }

    #[tokio::test]
    async fn fts_capability_exercises_insert_update_delete_score_and_highlight() {
        let (_directory, store, _path) = store("fts-capability").await;
        store.initialize().await.expect("initialize");
        let capabilities = store.capability_report().await.expect("capability report");
        let fts = capabilities
            .iter()
            .find(|item| item.capability == "fts")
            .expect("fts capability row");
        assert!(fts.available, "Turso FTS capability 不可用: {}", fts.detail);

        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO retrieval_documents(id, source_kind, content, content_hash, created_at, updated_at) VALUES ('doc_fts', 'test', 'alpha beta', 'h1', 1, 1)",
                (),
            )
            .await
            .expect("fts insert");
        let mut rows = connection
            .query(
                "SELECT fts_score(content, ?1), fts_highlight(content, '<b>', '</b>', ?1) FROM retrieval_documents WHERE fts_match(content, ?1)",
                ["alpha"],
            )
            .await
            .expect("fts query");
        let row = rows.next().await.expect("fts row").expect("fts match");
        let score = match row.get_value(0).expect("score") {
            turso::Value::Real(value) => value,
            turso::Value::Integer(value) => value as f64,
            value => panic!("unexpected fts score: {value:?}"),
        };
        assert!(score.is_finite(), "fts score was {score}");
        let highlighted =
            text_value(row.get_value(1).expect("highlight"), "highlight").expect("highlight text");
        assert!(highlighted.contains("<b>alpha</b>"));
        connection
            .execute(
                "UPDATE retrieval_documents SET content='gamma', updated_at=2 WHERE id='doc_fts'",
                (),
            )
            .await
            .expect("fts update");
        let mut rows = connection
            .query(
                "SELECT COUNT(*) FROM retrieval_documents WHERE fts_match(content, 'alpha')",
                (),
            )
            .await
            .expect("fts post-update query");
        let row = rows
            .next()
            .await
            .expect("fts update count row")
            .expect("fts update count");
        assert_eq!(
            integer_value(
                row.get_value(0).expect("fts update count"),
                "fts update count"
            )
            .expect("fts update count integer"),
            0
        );
        connection
            .execute("DELETE FROM retrieval_documents WHERE id='doc_fts'", ())
            .await
            .expect("fts delete");
        let mut rows = connection
            .query(
                "SELECT COUNT(*) FROM retrieval_documents WHERE fts_match(content, 'alpha')",
                (),
            )
            .await
            .expect("fts post-delete query");
        let row = rows
            .next()
            .await
            .expect("fts count row")
            .expect("fts count");
        assert_eq!(
            integer_value(row.get_value(0).expect("fts count"), "fts count")
                .expect("fts count integer"),
            0
        );
    }

    #[tokio::test]
    async fn ddl_foreign_key_and_import_journal_primitives_are_transactional() {
        let (_directory, store, _path) = store("ddl-capability").await;
        store.initialize().await.expect("initialize");
        let mut connection = store.connection().await.expect("connection");
        connection
            .execute(
                "CREATE TABLE capability_ddl(id INTEGER PRIMARY KEY, value TEXT)",
                (),
            )
            .await
            .expect("create table");
        connection
            .execute("ALTER TABLE capability_ddl ADD COLUMN note TEXT", ())
            .await
            .expect("alter table");
        let transaction = connection
            .transaction_with_behavior(turso::transaction::TransactionBehavior::Immediate)
            .await
            .expect("begin transaction");
        transaction
            .execute(
                "INSERT INTO import_journal(id, source_kind, source_path, snapshot_fingerprint, phase, manifest_json, created_at, updated_at) VALUES ('ij_test', 'jsonl', 'input.jsonl', 'fnv64:test', 'prepared', '{}', 1, 1)",
                (),
            )
            .await
            .expect("journal row");
        transaction
            .execute(
                "INSERT INTO attachment_staging(id, journal_id, attachment_id, source_rel_path, staged_rel_path, expected_size_bytes, created_at, updated_at) VALUES ('as_test', 'ij_test', 'a_test', 'a.txt', 'a.txt', 0, 1, 1)",
                (),
            )
            .await
            .expect("attachment staging row");
        transaction
            .execute(
                "UPDATE projection_maintenance_owner SET owner='tester', lease_token='lease', mode='rebuild', lease_expires_at=10, fence_epoch=1, started_at=1, last_heartbeat_at=1, updated_at=1 WHERE singleton=1",
                (),
            )
            .await
            .expect("maintenance owner row");
        transaction.commit().await.expect("commit");

        let mut rows = connection
            .query("SELECT phase FROM import_journal WHERE id='ij_test'", ())
            .await
            .expect("journal query");
        let row = rows
            .next()
            .await
            .expect("journal row")
            .expect("journal result");
        assert_eq!(
            text_value(row.get_value(0).expect("phase"), "phase").expect("phase text"),
            "prepared"
        );
    }

    #[tokio::test]
    async fn board_guard_trigger_and_composite_ontology_keys_are_exercised() {
        let (_directory, store, _path) = store("ontology-capability").await;
        store.initialize().await.expect("initialize");
        store
            .create_task(
                "default",
                create_input("t_ontology", Some("ontology-task"), "Ontology task"),
            )
            .await
            .expect("default task");

        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_other', 'other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("other board");
        drop(connection);
        store
            .create_task(
                "other",
                create_input("t_other", Some("other-task"), "Other task"),
            )
            .await
            .expect("other task");

        let connection = store.connection().await.expect("connection");
        let trigger_error = connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, kind, actor, payload_json, created_at) VALUES ('e_guard_mismatch', 'b_default', 't_other', 'test.guard', 'tester', '{}', 1)",
                (),
            )
            .await
            .expect_err("board guard trigger must reject cross-board task");
        assert!(
            trigger_error
                .to_string()
                .contains("task_events reference board mismatch")
        );

        connection
            .execute(
                "INSERT INTO labels(id, board_id, name, created_at, updated_at) VALUES ('l_ontology', 'b_default', 'Ontology', 1, 1)",
                (),
            )
            .await
            .expect("ontology label");
        connection
            .execute(
                "INSERT INTO label_semantic_proposals(id, board_id, task_id, name, created_by, created_at, updated_at) VALUES ('lp_ontology', 'b_default', 't_ontology', 'New ontology label', 'tester', 1, 1)",
                (),
            )
            .await
            .expect("semantic proposal composite parent");
        connection
            .execute(
                "INSERT INTO label_ontology_observations(id, board_id, task_id, task_ref_snapshot, task_snapshot_json, capture_fingerprint, created_by, created_by_type, created_at) VALUES ('lor_ontology', 'b_default', 't_ontology', 'kanban-tool#1', '{}', 'capture-ontology', 'tester', 'user', 1)",
                (),
            )
            .await
            .expect("ontology observation composite parent");
        connection
            .execute(
                "INSERT INTO label_ontology_signals(id, board_id, observation_id, kind, proposed_action, rationale, signal_key, created_at, updated_at) VALUES ('los_ontology', 'b_default', 'lor_ontology', 'vocabulary_gap', 'observe', 'test signal', 'signal-ontology', 1, 1)",
                (),
            )
            .await
            .expect("ontology signal composite parent");
        connection
            .execute(
                "INSERT INTO label_ontology_observations(id, board_id, task_id, task_ref_snapshot, task_snapshot_json, capture_fingerprint, created_by, created_by_type, created_at) VALUES ('lor_ontology_two', 'b_default', 't_ontology', 'kanban-tool#1', '{}', 'capture-ontology-two', 'tester', 'agent', 2)",
                (),
            )
            .await
            .expect("second ontology observation");
        connection
            .execute(
                "INSERT INTO label_ontology_signals(id, board_id, observation_id, kind, proposed_action, rationale, signal_key, created_at, updated_at) VALUES ('los_ontology_two', 'b_default', 'lor_ontology_two', 'vocabulary_gap', 'observe', 'same key in another observation', 'signal-ontology', 2, 2)",
                (),
            )
            .await
            .expect("signal key is scoped to one observation");
        connection
            .execute(
                "INSERT INTO label_ontology_actions(id, board_id, action_type, reason, result_proposal_id, created_by, created_by_type, created_at) VALUES ('loa_ontology', 'b_default', 'create_label_proposal', 'test action', 'lp_ontology', 'tester', 'user', 1)",
                (),
            )
            .await
            .expect("ontology action composite parent");
        connection
            .execute(
                "INSERT INTO label_ontology_action_signals(board_id, action_id, signal_id, created_at) VALUES ('b_default', 'loa_ontology', 'los_ontology', 1)",
                (),
            )
            .await
            .expect("ontology action signal link");
        connection
            .execute(
                "INSERT INTO label_ontology_actions(id, board_id, parent_action_id, action_type, reason, validation_status, validation_requirement, created_by, created_by_type, agent_type, created_at) VALUES ('loa_validate', 'b_default', 'loa_ontology', 'validate', 'partial validation', 'partial', 'required', 'tester', 'agent', 'luna-max', 2)",
                (),
            )
            .await
            .expect("ontology partial validation action");

        connection
            .execute(
                "INSERT INTO signal_observations(id, board_id, task_id, actor, created_at) VALUES ('obs_ontology', 'b_default', 't_ontology', 'tester', 1)",
                (),
            )
            .await
            .expect("signal observation composite parent");
        connection
            .execute(
                "INSERT INTO signals(id, board_id, observation_id, kind, title, summary, created_at, updated_at) VALUES ('sig_ontology', 'b_default', 'obs_ontology', 'ontology', 'Ontology signal', 'Signal summary', 1, 1)",
                (),
            )
            .await
            .expect("signal composite parent");

        // create_task 已经写入 canonical entity；这里确认复用同一 board-local 实体，避免重复插入。
        let mut entity_rows = connection
            .query(
                "SELECT COUNT(*) FROM entities WHERE uri='kb://task/t_ontology' AND board_id='b_default'",
                (),
            )
            .await
            .expect("projection entity query");
        let entity_row = entity_rows
            .next()
            .await
            .expect("projection entity row")
            .expect("projection entity result");
        assert_eq!(
            integer_value(
                entity_row.get_value(0).expect("projection entity count"),
                "projection entity count",
            )
            .expect("projection entity count integer"),
            1
        );
        connection
            .execute(
                "INSERT INTO projection_jobs(source_event_id, target, entity_uri, board_id, operation, payload_json, status, attempts, lease_owner, lease_token, lease_expires_at, fence_epoch, generation, next_attempt_at, created_at, updated_at) VALUES (NULL, 'fts', 'kb://task/t_ontology', 'b_default', 'upsert', '{}', 'running', 1, 'worker', 'claim', 10, 1, 'gen-1', 11, 1, 1)",
                (),
            )
            .await
            .expect("outbox worker claim fields");
    }

    #[tokio::test]
    async fn attachment_and_knowledge_guards_reject_path_escape_and_cross_board_links() {
        let (_directory, store, _path) = store("knowledge-guards").await;
        store.initialize().await.expect("initialize");
        store
            .create_task(
                "default",
                create_input("t_default_guard", Some("guard-default"), "Default"),
            )
            .await
            .expect("default task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_guard_other', 'guard-other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("other board");
        drop(connection);
        store
            .create_task(
                "guard-other",
                create_input("t_other_guard", Some("guard-other"), "Other"),
            )
            .await
            .expect("other task");

        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_attachments(id, board_id, task_id, filename, rel_path, size_bytes, created_by, created_at) VALUES ('a_safe', 'b_default', 't_default_guard', 'safe.txt', 'attachments/safe.txt', 1, 'tester', 1)",
                (),
            )
            .await
            .expect("safe attachment");
        let insert_error = connection
            .execute(
                "INSERT INTO task_attachments(id, board_id, task_id, filename, rel_path, size_bytes, created_by, created_at) VALUES ('a_escape', 'b_default', 't_default_guard', 'escape.txt', '../escape.txt', 1, 'tester', 1)",
                (),
            )
            .await
            .expect_err("attachment insert must reject parent traversal");
        assert!(insert_error.to_string().contains("rel_path escapes"));
        let update_error = connection
            .execute(
                "UPDATE task_attachments SET rel_path='attachments/../../escape.txt' WHERE id='a_safe'",
                (),
            )
            .await
            .expect_err("attachment update must reject parent traversal");
        assert!(update_error.to_string().contains("rel_path escapes"));

        // 两次 create_task 已经分别建立 default/guard-other 的 canonical entity。
        let relation_error = connection
            .execute(
                "INSERT INTO entity_relations(subject_uri, predicate, object_uri, graph_uri, board_id, created_at, updated_at) VALUES ('kb://task/t_default_guard', 'depends_on', 'kb://task/t_other_guard', 'kb://graph/b_default', 'b_default', 1, 1)",
                (),
            )
            .await
            .expect_err("cross-board relation must fail");
        assert!(
            relation_error
                .to_string()
                .contains("entity_relations reference board mismatch")
        );
        let document_error = connection
            .execute(
                "INSERT INTO retrieval_documents(id, board_id, entity_uri, source_kind, content, content_hash, created_at, updated_at) VALUES ('doc_cross', 'b_default', 'kb://task/t_other_guard', 'task', 'other', 'hash', 1, 1)",
                (),
            )
            .await
            .expect_err("cross-board retrieval document must fail");
        assert!(
            document_error
                .to_string()
                .contains("retrieval_documents entity board mismatch")
        );
    }
}
