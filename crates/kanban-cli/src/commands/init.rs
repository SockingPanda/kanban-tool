use kanban_protocol::{CliInitOutput, CliInitResult};

use crate::{config, error::CliFailure, output};

/// 创建项目选择文件；这个命令不会打开、初始化或迁移 Turso。
pub(crate) fn run(
    db: Option<&std::path::Path>,
    board: Option<&str>,
    json: bool,
) -> Result<(), CliFailure> {
    let (path, created, _configured_board) =
        config::init_project_config().map_err(CliFailure::from)?;
    let resolved_db = config::resolve_db_path(db).map_err(CliFailure::from)?;
    let board = config::resolve_board(board)
        .map_err(CliFailure::from)?
        .value;
    let result = CliInitResult {
        db_path: resolved_db.value.display().to_string(),
        board_id: "not_initialized".to_owned(),
        board_slug: board,
        config_path: Some(path.display().to_string()),
        created: Some(created),
    };
    let output_value = CliInitOutput::new(result);
    if json {
        output::print_json(&output_value);
    } else if created {
        println!(
            "已创建项目配置：{}\n当前 board：{}\n数据库由 `kanban serve` 负责：{}",
            path.display(),
            output_value.data.board_slug,
            output_value.data.db_path
        );
    } else {
        println!(
            "已复用项目配置：{}\n当前 board：{}\n数据库由 `kanban serve` 负责：{}",
            path.display(),
            output_value.data.board_slug,
            output_value.data.db_path
        );
    }
    Ok(())
}
