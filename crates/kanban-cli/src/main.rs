mod args;
mod commands;
mod output;

fn main() {
    if let Err(error) = commands::app::run() {
        if let Some(kanban_error) = error
            .chain()
            .find_map(|cause| cause.downcast_ref::<kanban_core::KanbanError>())
        {
            eprintln!(
                "Error: {}",
                kanban_core::i18n::render_error(kanban_core::current_locale(), kanban_error)
            );
        } else {
            eprintln!("Error: {error:?}");
        }
        std::process::exit(1);
    }
}
