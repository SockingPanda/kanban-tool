mod args;
mod commands;
mod output;

fn main() -> anyhow::Result<()> {
    commands::app::run()
}
