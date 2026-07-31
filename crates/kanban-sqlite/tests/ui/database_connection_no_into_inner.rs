use kanban_sqlite::db::DatabaseConnection;

fn move_raw(connection: DatabaseConnection) {
    let _ = connection.into_inner();
}

fn main() {}
