use clap::{Args, Subcommand};
use kanban_protocol::{
    BackupReport, CheckpointReport, DoctorReport, ExportReport, ImportReport, MaintenanceRunReport,
    MaintenanceStatusReport, QueueStats, VacuumReport,
};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Args)]
pub(crate) struct PathArgs {
    /// 输出或输入路径；文件不得覆盖既有目标。
    #[arg(long, short = 'o')]
    pub(crate) path: String,
}

#[derive(Debug, Args)]
pub(crate) struct ImportArgs {
    #[arg(long, short = 'i')]
    pub(crate) path: String,
    /// 仅允许在已验证 backup 和 host 独占窗口内使用。
    #[arg(long)]
    pub(crate) replace: bool,
}

#[derive(Debug, Args)]
pub(crate) struct LegacyImportArgs {
    #[arg(long, short = 'i')]
    pub(crate) path: String,
    /// 将 legacy SQLite 附件路径映射到 canonical attachment root。
    #[arg(long)]
    pub(crate) attachment_root: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct StatsArgs {
    #[arg(long)]
    pub(crate) board: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct MaintenanceArgs {
    #[command(subcommand)]
    pub(crate) command: MaintenanceCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum MaintenanceCommand {
    Status,
    Run(MaintenanceRunArgs),
    Rebuild(MaintenanceRunArgs),
    Cleanup(MaintenanceRunArgs),
}

#[derive(Debug, Args)]
pub(crate) struct MaintenanceRunArgs {
    #[arg(long)]
    pub(crate) owner: Option<String>,
}

pub(crate) async fn doctor(ctx: &CliContext) -> Result<(), CliFailure> {
    emit(ctx.json, ctx.client()?.doctor().await?, "doctor")
}
pub(crate) async fn checkpoint(ctx: &CliContext) -> Result<(), CliFailure> {
    emit(ctx.json, ctx.client()?.checkpoint().await?, "checkpoint")
}
pub(crate) async fn backup(ctx: &CliContext, args: &PathArgs) -> Result<(), CliFailure> {
    emit(ctx.json, ctx.client()?.backup(&args.path).await?, "backup")
}
pub(crate) async fn export(ctx: &CliContext, args: &PathArgs) -> Result<(), CliFailure> {
    emit(ctx.json, ctx.client()?.export(&args.path).await?, "export")
}
pub(crate) async fn import(ctx: &CliContext, args: &ImportArgs) -> Result<(), CliFailure> {
    emit(
        ctx.json,
        ctx.client()?.import(&args.path, args.replace).await?,
        "import",
    )
}
pub(crate) async fn import_v30(
    ctx: &CliContext,
    args: &LegacyImportArgs,
) -> Result<(), CliFailure> {
    emit(
        ctx.json,
        ctx.client()?
            .import_legacy_sqlite_v30(&args.path, args.attachment_root.clone())
            .await?,
        "import-v30",
    )
}
pub(crate) async fn vacuum(ctx: &CliContext) -> Result<(), CliFailure> {
    emit(ctx.json, ctx.client()?.vacuum().await?, "vacuum")
}
pub(crate) async fn stats(ctx: &CliContext, args: &StatsArgs) -> Result<(), CliFailure> {
    emit(
        ctx.json,
        ctx.client()?
            .stats(args.board.as_deref().unwrap_or(&ctx.board))
            .await?,
        "stats",
    )
}

pub(crate) async fn maintenance(
    ctx: &CliContext,
    args: &MaintenanceArgs,
) -> Result<(), CliFailure> {
    match &args.command {
        MaintenanceCommand::Status => emit(
            ctx.json,
            ctx.client()?.maintenance_status().await?,
            "maintenance status",
        ),
        MaintenanceCommand::Run(args) => emit(
            ctx.json,
            ctx.client()?
                .maintenance_run(args.owner.clone(), None)
                .await?,
            "maintenance run",
        ),
        MaintenanceCommand::Rebuild(args) => emit(
            ctx.json,
            ctx.client()?
                .maintenance_rebuild(args.owner.clone())
                .await?,
            "maintenance rebuild",
        ),
        MaintenanceCommand::Cleanup(args) => emit(
            ctx.json,
            ctx.client()?
                .maintenance_cleanup(args.owner.clone())
                .await?,
            "maintenance cleanup",
        ),
    }
}

fn emit<T: serde::Serialize + std::fmt::Debug>(
    json: bool,
    value: T,
    label: &str,
) -> Result<(), CliFailure> {
    if json {
        output::print_json(&serde_json::json!({"data": value}));
    } else {
        println!("{label}: {value:?}");
    }
    Ok(())
}

#[allow(dead_code)]
fn _typed_output_witness(
    _: (
        DoctorReport,
        CheckpointReport,
        BackupReport,
        ExportReport,
        ImportReport,
        VacuumReport,
        MaintenanceStatusReport,
        MaintenanceRunReport,
        QueueStats,
    ),
) {
}
