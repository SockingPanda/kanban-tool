#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn idempotency_and_board_column_constraints_are_enforced() {
        let (_directory, store, _path) = store("constraints").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");

        connection
            .execute(
                "INSERT INTO tasks(id, board_id, seq, idempotency_key, title, status, created_by, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 'todo', ?6, ?7, ?7)",
                ("t_one", "b_default", 1_i64, "client-1", "One", "test", 1_i64),
            )
            .await
            .expect("insert first task");
        let duplicate_idempotency = connection
            .execute(
                "INSERT INTO tasks(id, board_id, seq, idempotency_key, title, status, created_by, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 'todo', ?6, ?7, ?7)",
                ("t_two", "b_default", 2_i64, "client-1", "Two", "test", 2_i64),
            )
            .await;
        assert!(
            duplicate_idempotency.is_err(),
            "task idempotency must be unique per board"
        );

        let duplicate_column = connection
            .execute(
                "INSERT INTO board_columns(id, board_id, status, title, position, hidden, created_at, updated_at) VALUES (?1, ?2, 'todo', 'Duplicate', ?3, 0, ?4, ?4)",
                ("col_duplicate", "b_default", 200_i64, 2_i64),
            )
            .await;
        assert!(
            duplicate_column.is_err(),
            "board status columns must be unique"
        );
    }
}
