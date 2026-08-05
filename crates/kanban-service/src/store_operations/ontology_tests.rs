#[cfg(test)]
mod tests {
    use crate::test_support::*;
    use crate::{
        OntologyActorInput, OntologyApplyAtomInput, OntologyRevertInput, UpsertLabelSemanticsInput,
    };

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
