#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn list_expired_claims_is_board_isolated_and_excludes_archived_records() {
        let (_directory, store, _path) = store("reclaim-expired-isolation").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_reclaim_other', 'reclaim-other', 'Reclaim other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let other = store
            .create_task(
                "reclaim-other",
                create_input(
                    "t_reclaim_other",
                    Some("reclaim-other"),
                    "Other board reclaim",
                ),
            )
            .await
            .expect("create other-board task");
        store
            .mark_execution_plan_not_required(
                &other.id,
                plan_input("No plan", "planner", "e_reclaim_other_plan", 100),
            )
            .await
            .expect("mark other plan");
        store
            .promote_task(
                &other.id,
                promote_input(0, "promoter", "e_reclaim_other_promote", 200),
            )
            .await
            .expect("promote other task");
        let other_claim = store
            .claim_task(
                &other.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_other",
                    "r_reclaim_other",
                    "e_reclaim_other_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim other task");
        assert!(
            store
                .list_expired_claims("default", 500)
                .await
                .expect("list default expired claims")
                .is_empty()
        );
        let other_expired = store
            .list_expired_claims("reclaim-other", 500)
            .await
            .expect("list other expired claims");
        assert_eq!(other_expired.len(), 1);
        assert_eq!(other_expired[0].id, other.id);
        assert_eq!(other_expired[0].board_id, "b_reclaim_other");
        assert_eq!(other_expired[0].lock_version, other_claim.task.lock_version);

        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 600 WHERE id = ?1",
                [other.id.as_str()],
            )
            .await
            .expect("archive other task");
        assert!(
            store
                .list_expired_claims("reclaim-other", 700)
                .await
                .expect("list archived task claims")
                .is_empty()
        );
        let archived = store
            .reclaim_expired_task(
                &other.id,
                reclaim_input(
                    other_claim.task.lock_version,
                    "dispatcher",
                    "e_reclaim_archived",
                    "ready",
                    1,
                    "claim expired",
                    700,
                ),
            )
            .await
            .expect("archived task must be skipped");
        assert_eq!(archived, None);
    }
}
