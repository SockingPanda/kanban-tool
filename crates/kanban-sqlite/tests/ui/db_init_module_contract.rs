#![allow(unused_imports)]

use kanban_sqlite::{
    db::{
        DatabaseConnection, connect, connect_existing_read_only, connect_file, default_pragmas,
    },
    init::init_database,
};
use rusqlite::Connection;

fn connection_contract(connection: &DatabaseConnection) -> &Connection {
    connection
}

fn main() {}

fn guarded_read_only_constructor_is_stable() {
    let _ = connect_existing_read_only;
}
