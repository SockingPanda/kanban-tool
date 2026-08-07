use clap::Subcommand;
use kanban_protocol::{CliIndexDoctorOutput, CliIndexStatusOutput};

use crate::{context::CliContext, error::CliFailure, output};

#[derive(Debug, Subcommand)]
#[command(arg_required_else_help = true)]
pub(crate) enum IndexCommand {
    /// 查看搜索 index 状态。
    Status,
    /// 诊断搜索 index 一致性。
    Doctor,
    /// 从 canonical 任务重建搜索 index。
    Rebuild,
    /// 同步待处理的搜索 projection 工作。
    Sync,
}

pub(crate) fn run(ctx: &CliContext, command: &IndexCommand) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    match command {
        IndexCommand::Status => show_status(ctx, client.search_status(&ctx.board)?, false),
        IndexCommand::Doctor => show_status(ctx, client.search_status(&ctx.board)?, true),
        IndexCommand::Rebuild => show_status(ctx, client.rebuild_search_index(&ctx.board)?, false),
        IndexCommand::Sync => show_status(ctx, client.sync_search_index(&ctx.board)?, false),
    }
}

fn show_status(
    ctx: &CliContext,
    response: kanban_protocol::SearchStatusResponse,
    doctor: bool,
) -> Result<(), CliFailure> {
    if ctx.json {
        if doctor {
            output::print_json(&CliIndexDoctorOutput::new(response.data));
        } else {
            output::print_json(&CliIndexStatusOutput::new(response.data));
        }
    } else {
        let status = response.data;
        println!(
            "search backend={}；derived_index={}；stale={}；last_event_id={:?}；lag={:?}：{}",
            status.backend,
            status.derived_index,
            status.stale,
            status.last_event_id,
            status.index_lag_events,
            status.message
        );
    }
    Ok(())
}
