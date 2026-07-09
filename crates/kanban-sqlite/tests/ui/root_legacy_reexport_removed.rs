use kanban_sqlite::{connect_file, create_task, init_database, CreateTask};

fn main() {
    let _ = connect_file;
    let _ = create_task;
    let _ = init_database;
    let _ = CreateTask::ready("legacy root path");
}
