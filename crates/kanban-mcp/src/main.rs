mod shared;
mod tools;

use rmcp::{
    ServerHandler, ServiceExt, handler::server::router::tool::ToolRouter, tool_handler,
    transport::stdio,
};
use shared::KanbanMcp;

#[tool_handler]
impl ServerHandler for KanbanMcp {}

impl KanbanMcp {
    fn tool_router() -> ToolRouter<Self> {
        Self::board_tools()
            + Self::task_tools()
            + Self::comment_tools()
            + Self::dependency_tools()
            + Self::event_tools()
            + Self::run_tools()
            + Self::step_tools()
            + Self::lifecycle_tools()
            + Self::ontology_tools()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let service = KanbanMcp::from_env()?.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::KanbanMcp;

    #[test]
    fn tool_inventory_is_stable() {
        let names: Vec<_> = KanbanMcp::tool_router()
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();

        assert_eq!(
            names,
            vec![
                "board_archive",
                "board_create",
                "board_list",
                "board_show",
                "comment_create",
                "comment_list",
                "dependency_create",
                "dependency_list",
                "dependency_remove",
                "event_list",
                "label_atom_explain",
                "label_atom_index_query",
                "label_atom_index_rebuild",
                "label_atom_index_status",
                "label_atoms_list",
                "label_ontology_action",
                "label_ontology_apply_atom",
                "label_ontology_observe",
                "label_ontology_quality",
                "label_ontology_revert",
                "label_ontology_review",
                "label_ontology_signal_show",
                "label_ontology_signals",
                "label_ontology_validate",
                "label_proposal_accept",
                "label_proposal_reject",
                "label_proposal_show",
                "label_proposals_list",
                "label_propose",
                "label_semantics_delete",
                "label_semantics_list",
                "label_semantics_show",
                "label_semantics_upsert",
                "label_suggest",
                "run_list",
                "run_log",
                "run_show",
                "step_create",
                "step_done",
                "step_list",
                "step_remove",
                "step_reopen",
                "step_skip",
                "step_update",
                "task_archive",
                "task_block",
                "task_claim",
                "task_create",
                "task_done",
                "task_heartbeat",
                "task_list",
                "task_plan_not_required",
                "task_promote",
                "task_reclaim",
                "task_release",
                "task_reopen",
                "task_review",
                "task_show",
                "task_specify",
                "task_unblock",
                "task_update",
            ]
        );
    }
}
