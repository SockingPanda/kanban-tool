use kanban_sqlite::db::DatabaseConnection;
use rusqlite::Connection;

fn raw_mut(connection: &mut DatabaseConnection) -> &mut Connection {
    connection
}

fn main() {}
