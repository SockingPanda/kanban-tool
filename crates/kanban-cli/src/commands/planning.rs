//! 专用模块/迭代命令；selector 与业务调用由共享 client 负责。
use crate::{context::CliContext, error::CliFailure, output};
use clap::{Args, Subcommand, ValueEnum};
use kanban_protocol::*;
use std::collections::BTreeMap;

#[derive(Debug, Args)]
pub(crate) struct Versions {
    /// 稳定请求标识；重试时保持参数和 actor 相同。
    #[arg(long)]
    pub(crate) request_id: Option<String>,
    #[arg(long)]
    pub(crate) expected_version: Option<i64>,
    /// 可选的旧、新端点双版本映射。
    #[arg(long, value_name = "JSON")]
    pub(crate) expected_versions: Option<String>,
    #[arg(long)]
    pub(crate) expected_catalog_version: Option<i64>,
}
impl Versions {
    fn endpoints(&self) -> Result<BTreeMap<String, ApiObjectVersion>, CliFailure> {
        self.expected_versions
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .map(Option::unwrap_or_default)
            .map_err(|e| CliFailure {
                code: "invalid_input",
                message: format!("expected-versions 无效: {e}"),
                exit_code: 2,
            })
    }
}
#[derive(Debug, Args)]
pub(crate) struct CreateVersions {
    /// 稳定请求标识；重试时保持参数和 actor 相同。
    #[arg(long)]
    request_id: Option<String>,
    /// 可选的关系端点双版本映射。
    #[arg(long, value_name = "JSON")]
    expected_versions: Option<String>,
    #[arg(long)]
    expected_catalog_version: Option<i64>,
}
impl CreateVersions {
    fn endpoints(&self) -> Result<BTreeMap<String, ApiObjectVersion>, CliFailure> {
        Versions {
            request_id: None,
            expected_version: None,
            expected_versions: self.expected_versions.clone(),
            expected_catalog_version: None,
        }
        .endpoints()
    }
}
#[derive(Debug, Args)]
pub(crate) struct Page {
    #[arg(long, default_value_t = 100)]
    limit: usize,
    #[arg(long, default_value_t = 0)]
    offset: usize,
}
#[derive(Debug, Args)]
pub(crate) struct Show {
    id: String,
}
#[derive(Debug, Args)]
pub(crate) struct Action {
    id: String,
    #[command(flatten)]
    versions: Versions,
}
#[derive(Debug, Args)]
pub(crate) struct Member {
    id: String,
    task_ref: String,
    #[command(flatten)]
    versions: Versions,
}
#[derive(Debug, Args)]
pub(crate) struct Members {
    id: String,
    #[command(flatten)]
    page: Page,
}
#[derive(Debug, Args)]
pub(crate) struct Close {
    id: String,
    #[arg(long)]
    carry_to: Option<String>,
    #[command(flatten)]
    versions: Versions,
}
#[derive(Debug, Subcommand)]
pub(crate) enum MemberCommand {
    List(Members),
    Add(Member),
    Remove(Member),
}
#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum CycleStatus {
    Planned,
    Active,
    Completed,
    Cancelled,
}
impl From<CycleStatus> for ApiCycleStatus {
    fn from(v: CycleStatus) -> Self {
        match v {
            CycleStatus::Planned => Self::Planned,
            CycleStatus::Active => Self::Active,
            CycleStatus::Completed => Self::Completed,
            CycleStatus::Cancelled => Self::Cancelled,
        }
    }
}
#[derive(Debug, Args)]
pub(crate) struct ModuleList {
    #[arg(long = "query")]
    q: Option<String>,
    #[arg(long)]
    include_archived: bool,
    #[command(flatten)]
    page: Page,
    #[arg(long = "parent")]
    parent_id: Option<String>,
}
#[derive(Debug, Args)]
pub(crate) struct ModuleCreate {
    title: String,
    #[arg(long)]
    body: Option<String>,
    #[arg(long = "parent")]
    parent_id: Option<String>,
    #[command(flatten)]
    versions: CreateVersions,
}
#[derive(Debug, Args)]
pub(crate) struct ModuleUpdate {
    id: String,
    #[arg(long)]
    title: Option<String>,
    #[arg(long, conflicts_with = "clear_body")]
    body: Option<String>,
    #[arg(long)]
    clear_body: bool,
    #[arg(long = "parent", conflicts_with = "clear_parent")]
    parent_id: Option<String>,
    #[arg(long)]
    clear_parent: bool,
    #[command(flatten)]
    versions: Versions,
}
#[derive(Debug, Subcommand)]
pub(crate) enum ModuleCommand {
    /// 分页查询；正文通过 show 读取。
    List(ModuleList),
    Show(Show),
    Create(ModuleCreate),
    Update(ModuleUpdate),
    Archive(Action),
    Restore(Action),
    Overview(Show),
    Task {
        #[command(subcommand)]
        command: MemberCommand,
    },
}
pub(crate) async fn run_module(
    ctx: &CliContext,
    command: &ModuleCommand,
) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    match command {
        ModuleCommand::List(a) => {
            let response = client
                .list_modules(
                    &ctx.board,
                    &ListModulesQuery {
                        q: a.q.clone(),
                        include_archived: a.include_archived,
                        limit: a.page.limit,
                        offset: a.page.offset,
                        parent_id: a.parent_id.clone(),
                    },
                )
                .await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                for item in &response.data {
                    println!("{} {}", item.id, item.title);
                }
                println!("total={} offset={}", response.total, response.offset);
            }
        }
        ModuleCommand::Show(a) => {
            let response = client.get_module(&ctx.board, &a.id).await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                println!("{} {}", response.data.id, response.data.title);
                if let Some(body) = &response.data.body {
                    println!("{body}");
                }
            }
        }
        ModuleCommand::Create(a) => {
            let response = client
                .create_module(
                    &ctx.board,
                    &CreateModuleRequest {
                        title: a.title.clone(),
                        body: a.body.clone(),
                        parent_id: a.parent_id.clone(),
                        request_id: a.versions.request_id.clone(),
                        actor: None,
                        expected_versions: a.versions.endpoints()?,
                        expected_catalog_version: a.versions.expected_catalog_version,
                    },
                )
                .await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                println!(
                    "{} {} version={} request_id={}",
                    response.data.object.id,
                    response.data.object.title,
                    response.data.object.version,
                    response.data.receipt.request_id
                );
            }
        }
        ModuleCommand::Update(a) => {
            let response = client
                .update_module(
                    &ctx.board,
                    &a.id,
                    &UpdateModuleRequest {
                        title: a.title.clone(),
                        body: if a.clear_body {
                            Some(None)
                        } else {
                            a.body.clone().map(Some)
                        },
                        parent_id: if a.clear_parent {
                            Some(None)
                        } else {
                            a.parent_id.clone().map(Some)
                        },
                        request_id: a.versions.request_id.clone(),
                        actor: None,
                        expected_versions: a.versions.endpoints()?,
                        expected_catalog_version: a.versions.expected_catalog_version,
                        expected_version: a.versions.expected_version,
                    },
                )
                .await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                println!(
                    "{} {} version={} request_id={}",
                    response.data.object.id,
                    response.data.object.title,
                    response.data.object.version,
                    response.data.receipt.request_id
                );
            }
        }
        ModuleCommand::Archive(a) => {
            let response = client
                .archive_module(
                    &ctx.board,
                    &a.id,
                    &PlanningActionRequest {
                        request_id: a.versions.request_id.clone(),
                        actor: None,
                        expected_versions: a.versions.endpoints()?,
                        expected_catalog_version: a.versions.expected_catalog_version,
                        expected_version: a.versions.expected_version,
                    },
                )
                .await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                println!(
                    "{} {} version={} request_id={}",
                    response.data.object.id,
                    response.data.object.title,
                    response.data.object.version,
                    response.data.receipt.request_id
                );
            }
        }
        ModuleCommand::Restore(a) => {
            let response = client
                .restore_module(
                    &ctx.board,
                    &a.id,
                    &PlanningActionRequest {
                        request_id: a.versions.request_id.clone(),
                        actor: None,
                        expected_versions: a.versions.endpoints()?,
                        expected_catalog_version: a.versions.expected_catalog_version,
                        expected_version: a.versions.expected_version,
                    },
                )
                .await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                println!(
                    "{} {} version={} request_id={}",
                    response.data.object.id,
                    response.data.object.title,
                    response.data.object.version,
                    response.data.receipt.request_id
                );
            }
        }
        ModuleCommand::Overview(a) => {
            let response = client.get_module_overview(&ctx.board, &a.id).await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                println!(
                    "{} {} done={}/{} source={:?}",
                    response.data.object.id,
                    response.data.object.title,
                    response.data.progress.done,
                    response.data.progress.total,
                    response.data.source
                );
            }
        }
        ModuleCommand::Task { command } => match command {
            MemberCommand::List(a) => {
                let response = client
                    .list_module_tasks(
                        &ctx.board,
                        &a.id,
                        &PlanningMembersQuery {
                            limit: a.page.limit,
                            offset: a.page.offset,
                        },
                    )
                    .await?;
                if ctx.json {
                    output::print_json(&response);
                } else {
                    for task in &response.data {
                        println!("{} {:?} {}", task.id, task.status, task.title);
                    }
                    println!("total={} source={:?}", response.total, response.source);
                }
            }
            MemberCommand::Add(a) => {
                let response = client
                    .add_module_task(
                        &ctx.board,
                        &a.id,
                        &a.task_ref,
                        &PlanningActionRequest {
                            request_id: a.versions.request_id.clone(),
                            actor: None,
                            expected_versions: a.versions.endpoints()?,
                            expected_catalog_version: a.versions.expected_catalog_version,
                            expected_version: a.versions.expected_version,
                        },
                    )
                    .await?;
                if ctx.json {
                    output::print_json(&response);
                } else {
                    println!(
                        "{} request_id={}",
                        response.data.object.id, response.data.receipt.request_id
                    );
                }
            }
            MemberCommand::Remove(a) => {
                let response = client
                    .remove_module_task(
                        &ctx.board,
                        &a.id,
                        &a.task_ref,
                        &PlanningActionRequest {
                            request_id: a.versions.request_id.clone(),
                            actor: None,
                            expected_versions: a.versions.endpoints()?,
                            expected_catalog_version: a.versions.expected_catalog_version,
                            expected_version: a.versions.expected_version,
                        },
                    )
                    .await?;
                if ctx.json {
                    output::print_json(&response);
                } else {
                    println!(
                        "{} request_id={}",
                        response.data.object.id, response.data.receipt.request_id
                    );
                }
            }
        },
    }
    Ok(())
}
#[derive(Debug, Args)]
pub(crate) struct CycleList {
    #[arg(long = "query")]
    q: Option<String>,
    #[arg(long)]
    include_archived: bool,
    #[command(flatten)]
    page: Page,
    #[arg(long, value_enum)]
    status: Option<CycleStatus>,
}
#[derive(Debug, Args)]
pub(crate) struct CycleCreate {
    title: String,
    #[arg(long)]
    body: Option<String>,
    #[arg(long, help = "计划开始时间，毫秒时间戳")]
    starts_at: i64,
    #[arg(long, help = "计划结束时间，毫秒时间戳")]
    ends_at: i64,
    #[command(flatten)]
    versions: CreateVersions,
}
#[derive(Debug, Args)]
pub(crate) struct CycleUpdate {
    id: String,
    #[arg(long)]
    title: Option<String>,
    #[arg(long, conflicts_with = "clear_body")]
    body: Option<String>,
    #[arg(long)]
    clear_body: bool,
    #[arg(long)]
    starts_at: Option<i64>,
    #[arg(long)]
    ends_at: Option<i64>,
    #[command(flatten)]
    versions: Versions,
}
#[derive(Debug, Subcommand)]
pub(crate) enum CycleCommand {
    /// 分页查询；正文通过 show 读取。
    List(CycleList),
    Show(Show),
    Create(CycleCreate),
    Update(CycleUpdate),
    Archive(Action),
    Restore(Action),
    Overview(Show),
    Task {
        #[command(subcommand)]
        command: MemberCommand,
    },
    Start(Action),
    Close(Close),
    Cancel(Action),
}
pub(crate) async fn run_cycle(ctx: &CliContext, command: &CycleCommand) -> Result<(), CliFailure> {
    let client = ctx.client()?;
    match command {
        CycleCommand::List(a) => {
            let response = client
                .list_cycles(
                    &ctx.board,
                    &ListCyclesQuery {
                        q: a.q.clone(),
                        include_archived: a.include_archived,
                        limit: a.page.limit,
                        offset: a.page.offset,
                        status: a.status.map(Into::into),
                    },
                )
                .await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                for item in &response.data {
                    println!("{} {}", item.id, item.title);
                }
                println!("total={} offset={}", response.total, response.offset);
            }
        }
        CycleCommand::Show(a) => {
            let response = client.get_cycle(&ctx.board, &a.id).await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                println!("{} {}", response.data.id, response.data.title);
                if let Some(body) = &response.data.body {
                    println!("{body}");
                }
            }
        }
        CycleCommand::Create(a) => {
            let response = client
                .create_cycle(
                    &ctx.board,
                    &CreateCycleRequest {
                        title: a.title.clone(),
                        body: a.body.clone(),
                        starts_at: a.starts_at,
                        ends_at: a.ends_at,
                        request_id: a.versions.request_id.clone(),
                        actor: None,
                        expected_versions: a.versions.endpoints()?,
                        expected_catalog_version: a.versions.expected_catalog_version,
                    },
                )
                .await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                println!(
                    "{} {} version={} request_id={}",
                    response.data.object.id,
                    response.data.object.title,
                    response.data.object.version,
                    response.data.receipt.request_id
                );
            }
        }
        CycleCommand::Update(a) => {
            let response = client
                .update_cycle(
                    &ctx.board,
                    &a.id,
                    &UpdateCycleRequest {
                        title: a.title.clone(),
                        body: if a.clear_body {
                            Some(None)
                        } else {
                            a.body.clone().map(Some)
                        },
                        starts_at: a.starts_at,
                        ends_at: a.ends_at,
                        request_id: a.versions.request_id.clone(),
                        actor: None,
                        expected_versions: a.versions.endpoints()?,
                        expected_catalog_version: a.versions.expected_catalog_version,
                        expected_version: a.versions.expected_version,
                    },
                )
                .await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                println!(
                    "{} {} version={} request_id={}",
                    response.data.object.id,
                    response.data.object.title,
                    response.data.object.version,
                    response.data.receipt.request_id
                );
            }
        }
        CycleCommand::Archive(a) => {
            let response = client
                .archive_cycle(
                    &ctx.board,
                    &a.id,
                    &PlanningActionRequest {
                        request_id: a.versions.request_id.clone(),
                        actor: None,
                        expected_versions: a.versions.endpoints()?,
                        expected_catalog_version: a.versions.expected_catalog_version,
                        expected_version: a.versions.expected_version,
                    },
                )
                .await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                println!(
                    "{} {} version={} request_id={}",
                    response.data.object.id,
                    response.data.object.title,
                    response.data.object.version,
                    response.data.receipt.request_id
                );
            }
        }
        CycleCommand::Restore(a) => {
            let response = client
                .restore_cycle(
                    &ctx.board,
                    &a.id,
                    &PlanningActionRequest {
                        request_id: a.versions.request_id.clone(),
                        actor: None,
                        expected_versions: a.versions.endpoints()?,
                        expected_catalog_version: a.versions.expected_catalog_version,
                        expected_version: a.versions.expected_version,
                    },
                )
                .await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                println!(
                    "{} {} version={} request_id={}",
                    response.data.object.id,
                    response.data.object.title,
                    response.data.object.version,
                    response.data.receipt.request_id
                );
            }
        }
        CycleCommand::Overview(a) => {
            let response = client.get_cycle_overview(&ctx.board, &a.id).await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                println!(
                    "{} {} done={}/{} source={:?}",
                    response.data.object.id,
                    response.data.object.title,
                    response.data.progress.done,
                    response.data.progress.total,
                    response.data.source
                );
            }
        }
        CycleCommand::Start(a) => {
            let response = client
                .start_cycle(
                    &ctx.board,
                    &a.id,
                    &PlanningActionRequest {
                        request_id: a.versions.request_id.clone(),
                        actor: None,
                        expected_versions: a.versions.endpoints()?,
                        expected_catalog_version: a.versions.expected_catalog_version,
                        expected_version: a.versions.expected_version,
                    },
                )
                .await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                println!(
                    "{} {} version={} request_id={}",
                    response.data.object.id,
                    response.data.object.title,
                    response.data.object.version,
                    response.data.receipt.request_id
                );
            }
        }
        CycleCommand::Close(a) => {
            let response = client
                .close_cycle(
                    &ctx.board,
                    &a.id,
                    &CloseCycleRequest {
                        carry_to: a.carry_to.clone(),
                        request_id: a.versions.request_id.clone(),
                        actor: None,
                        expected_versions: a.versions.endpoints()?,
                        expected_catalog_version: a.versions.expected_catalog_version,
                        expected_version: a.versions.expected_version,
                    },
                )
                .await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                println!(
                    "{} {} version={} request_id={}",
                    response.data.object.id,
                    response.data.object.title,
                    response.data.object.version,
                    response.data.receipt.request_id
                );
            }
        }
        CycleCommand::Cancel(a) => {
            let response = client
                .cancel_cycle(
                    &ctx.board,
                    &a.id,
                    &PlanningActionRequest {
                        request_id: a.versions.request_id.clone(),
                        actor: None,
                        expected_versions: a.versions.endpoints()?,
                        expected_catalog_version: a.versions.expected_catalog_version,
                        expected_version: a.versions.expected_version,
                    },
                )
                .await?;
            if ctx.json {
                output::print_json(&response);
            } else {
                println!(
                    "{} {} version={} request_id={}",
                    response.data.object.id,
                    response.data.object.title,
                    response.data.object.version,
                    response.data.receipt.request_id
                );
            }
        }
        CycleCommand::Task { command } => match command {
            MemberCommand::List(a) => {
                let response = client
                    .list_cycle_tasks(
                        &ctx.board,
                        &a.id,
                        &PlanningMembersQuery {
                            limit: a.page.limit,
                            offset: a.page.offset,
                        },
                    )
                    .await?;
                if ctx.json {
                    output::print_json(&response);
                } else {
                    for task in &response.data {
                        println!("{} {:?} {}", task.id, task.status, task.title);
                    }
                    println!("total={} source={:?}", response.total, response.source);
                }
            }
            MemberCommand::Add(a) => {
                let response = client
                    .add_cycle_task(
                        &ctx.board,
                        &a.id,
                        &a.task_ref,
                        &PlanningActionRequest {
                            request_id: a.versions.request_id.clone(),
                            actor: None,
                            expected_versions: a.versions.endpoints()?,
                            expected_catalog_version: a.versions.expected_catalog_version,
                            expected_version: a.versions.expected_version,
                        },
                    )
                    .await?;
                if ctx.json {
                    output::print_json(&response);
                } else {
                    println!(
                        "{} request_id={}",
                        response.data.object.id, response.data.receipt.request_id
                    );
                }
            }
            MemberCommand::Remove(a) => {
                let response = client
                    .remove_cycle_task(
                        &ctx.board,
                        &a.id,
                        &a.task_ref,
                        &PlanningActionRequest {
                            request_id: a.versions.request_id.clone(),
                            actor: None,
                            expected_versions: a.versions.endpoints()?,
                            expected_catalog_version: a.versions.expected_catalog_version,
                            expected_version: a.versions.expected_version,
                        },
                    )
                    .await?;
                if ctx.json {
                    output::print_json(&response);
                } else {
                    println!(
                        "{} request_id={}",
                        response.data.object.id, response.data.receipt.request_id
                    );
                }
            }
        },
    }
    Ok(())
}
