use std::path::Path;

use kanban_sqlite::api::connect_existing_database_read_only;

fn main() {
    let _ = connect_existing_database_read_only(Path::new("database.db"));
}
