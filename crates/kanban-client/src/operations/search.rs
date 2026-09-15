use kanban_protocol::{
    SearchStatusResponse, SearchTasksByStatusResponse, SearchTasksQuery, SearchTasksResponse,
};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    pub async fn search_tasks(
        &self,
        query: &SearchTasksQuery,
    ) -> Result<SearchTasksResponse, ClientError> {
        let response: kanban_protocol::SearchTasksResponse = rpc!(
            self,
            search_tasks,
            SearchTasksRequest,
            (),
            query.clone(),
            ()
        )?;
        Ok(response)
    }

    pub async fn search_tasks_by_status(
        &self,
        query: &SearchTasksQuery,
    ) -> Result<SearchTasksByStatusResponse, ClientError> {
        let response: kanban_protocol::SearchTasksByStatusResponse = rpc!(
            self,
            search_tasks_by_status,
            SearchTasksByStatusRequest,
            (),
            query.clone(),
            ()
        )?;
        Ok(response)
    }

    pub async fn search_status(&self, board: &str) -> Result<SearchStatusResponse, ClientError> {
        let response: kanban_protocol::SearchStatusResponse = rpc!(
            self,
            search_status,
            SearchStatusRequest,
            (),
            kanban_protocol::BoardQuery {
                board: board.trim().to_owned()
            },
            ()
        )?;
        Ok(response)
    }

    pub async fn rebuild_search_index(
        &self,
        board: &str,
    ) -> Result<SearchStatusResponse, ClientError> {
        let response: kanban_protocol::SearchStatusResponse = rpc!(
            self,
            rebuild_search_index,
            RebuildSearchIndexRequest,
            (),
            kanban_protocol::BoardQuery {
                board: board.trim().to_owned()
            },
            ()
        )?;
        Ok(response)
    }

    pub async fn sync_search_index(
        &self,
        board: &str,
    ) -> Result<SearchStatusResponse, ClientError> {
        let response: kanban_protocol::SearchStatusResponse = rpc!(
            self,
            sync_search_index,
            SyncSearchIndexRequest,
            (),
            kanban_protocol::BoardQuery {
                board: board.trim().to_owned()
            },
            ()
        )?;
        Ok(response)
    }
}
