use kanban_protocol::{
    ConfirmSignalsResponse, GetSignalResponse, ListSignalsResponse, RecordSignalRequest,
    RecordSignalResponse, RejectSignalsResponse, ResolveSignalsResponse, ReviewSignalsRequest,
    ReviewSignalsResponse, SignalQuery, SupersedeSignalsResponse,
};

use crate::{KanbanClient, error::ClientError, transport::rpc};

impl KanbanClient {
    /// 记录一个通用 signal，并在请求中固定当前 client actor。
    pub async fn record_signal(
        &self,
        board: &str,
        request: &RecordSignalRequest,
    ) -> Result<RecordSignalResponse, ClientError> {
        let board = required_board(board)?;
        let mut request = request.clone();
        request.actor = Some(self.actor.clone());
        let response: kanban_protocol::RecordSignalResponse = rpc!(
            self,
            record_signal,
            RecordSignalRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response)
    }

    pub async fn list_signals(
        &self,
        board: &str,
        query: &SignalQuery,
    ) -> Result<ListSignalsResponse, ClientError> {
        let board = required_board(board)?;
        let response: kanban_protocol::ListSignalsResponse = rpc!(
            self,
            list_signals,
            ListSignalsRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            query.clone(),
            ()
        )?;
        Ok(response)
    }

    pub async fn review_signals(
        &self,
        board: &str,
        query: &SignalQuery,
    ) -> Result<ReviewSignalsResponse, ClientError> {
        let board = required_board(board)?;
        let response: kanban_protocol::ReviewSignalsResponse = rpc!(
            self,
            review_signals,
            ReviewSignalsRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            query.clone(),
            ()
        )?;
        Ok(response)
    }

    pub async fn get_signal(&self, signal_id: &str) -> Result<GetSignalResponse, ClientError> {
        let signal_id = required_signal_id(signal_id)?;
        let response: kanban_protocol::GetSignalResponse = rpc!(
            self,
            get_signal,
            GetSignalRequest,
            kanban_protocol::SignalPath {
                signal_id: signal_id.to_owned()
            },
            (),
            ()
        )?;
        Ok(response)
    }

    pub async fn confirm_signals(
        &self,
        board: &str,
        request: &ReviewSignalsRequest,
    ) -> Result<ConfirmSignalsResponse, ClientError> {
        let board = required_board(board)?;
        let mut request = request.clone();
        request.actor = Some(self.actor.clone());
        let response: kanban_protocol::ConfirmSignalsResponse = rpc!(
            self,
            confirm_signals,
            ConfirmSignalsRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response)
    }

    pub async fn reject_signals(
        &self,
        board: &str,
        request: &ReviewSignalsRequest,
    ) -> Result<RejectSignalsResponse, ClientError> {
        let board = required_board(board)?;
        let mut request = request.clone();
        request.actor = Some(self.actor.clone());
        let response: kanban_protocol::RejectSignalsResponse = rpc!(
            self,
            reject_signals,
            RejectSignalsRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response)
    }

    pub async fn resolve_signals(
        &self,
        board: &str,
        request: &ReviewSignalsRequest,
    ) -> Result<ResolveSignalsResponse, ClientError> {
        let board = required_board(board)?;
        let mut request = request.clone();
        request.actor = Some(self.actor.clone());
        let response: kanban_protocol::ResolveSignalsResponse = rpc!(
            self,
            resolve_signals,
            ResolveSignalsRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response)
    }

    pub async fn supersede_signals(
        &self,
        board: &str,
        request: &ReviewSignalsRequest,
    ) -> Result<SupersedeSignalsResponse, ClientError> {
        let board = required_board(board)?;
        let mut request = request.clone();
        request.actor = Some(self.actor.clone());
        let response: kanban_protocol::SupersedeSignalsResponse = rpc!(
            self,
            supersede_signals,
            SupersedeSignalsRequest,
            kanban_protocol::BoardLabelPath {
                board: board.to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response)
    }
}

fn required_board(board: &str) -> Result<&str, ClientError> {
    let board = board.trim();
    if board.is_empty() {
        return Err(ClientError::InvalidInput("必须提供 board".to_owned()));
    }
    Ok(board)
}

fn required_signal_id(signal_id: &str) -> Result<&str, ClientError> {
    let signal_id = signal_id.trim();
    if !signal_id.starts_with("sig_") || signal_id.len() <= 4 {
        return Err(ClientError::InvalidInput(
            "signal ID 必须是全局 sig_... ID".to_owned(),
        ));
    }
    Ok(signal_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn invalid_signal_identifiers_are_rejected_before_http() {
        let client = KanbanClient::new(crate::DEFAULT_SERVER_URL, "test").unwrap();
        assert_eq!(
            client.get_signal("task#1").await.unwrap_err().code(),
            "invalid_input"
        );
        assert_eq!(
            client
                .list_signals(
                    " ",
                    &SignalQuery {
                        status: Vec::new(),
                        kind: Vec::new(),
                        task_ref: None,
                        include_all: false,
                        limit: 100,
                    },
                )
                .await
                .unwrap_err()
                .code(),
            "invalid_input"
        );
    }
}
