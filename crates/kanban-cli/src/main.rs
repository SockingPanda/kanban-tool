use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "kb", version, about = "Local SQLite-backed Kanban work queue")]
struct Cli {
    #[arg(long, global = true)]
    db: Option<PathBuf>,
    #[arg(long, global = true, default_value = "default")]
    board: String,
    #[arg(long, global = true)]
    actor: Option<String>,
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Initialize the local kb database.
    Init {
        /// Re-run init even if the database already exists. Current init is idempotent.
        #[arg(long)]
        force: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init { force: _ } => {
            let db_path = cli.db.unwrap_or_else(default_db_path);
            let actor = cli.actor.unwrap_or_else(default_actor);
            let result = kanban_sqlite::init_database(&db_path, &actor)
                .with_context(|| format!("failed to initialize {}", db_path.display()))?;
            if cli.json {
                let payload = serde_json::json!({
                    "data": {
                        "db_path": result.db_path,
                        "board": result.board_slug,
                        "board_id": result.board_id,
                    }
                });
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                println!("Initialized kb database at {}", result.db_path.display());
                println!("Default board: {}", result.board_slug);
            }
        }
    }
    Ok(())
}

fn default_db_path() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kb")
        .join("kb.db")
}

fn default_actor() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "local".to_owned())
}
