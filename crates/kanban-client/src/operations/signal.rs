use kanban_protocol::{
    ConfirmSignalsResponse, GetSignalResponse, ListSignalsResponse, RecordSignalRequest,
    RecordSignalResponse, RejectSignalsResponse, ResolveSignalsResponse, ReviewSignalsRequest,
    ReviewSignalsResponse, SignalQuery, SignalWire, SupersedeSignalsResponse,
};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    /// 记录一个通用 signal，并在请求中固定当前 client actor。
    pub fn record_signal(
        &self,
        board: &str,
        request: &RecordSignalRequest,
    ) -> Result<RecordSignalResponse, ClientError> {
        let board = required_board(board)?;
        let mut request = request.clone();
        request.actor = Some(self.actor.clone());
        self.post(
            &format!("/api/v1/boards/{}/signals", encode_path_segment(board)),
            &request,
        )
    }

    pub fn list_signals(
        &self,
        board: &str,
        query: &SignalQuery,
    ) -> Result<ListSignalsResponse, ClientError> {
        let board = required_board(board)?;
        self.get(&signals_path(board, "signals", query))
    }

    pub fn review_signals(
        &self,
        board: &str,
        query: &SignalQuery,
    ) -> Result<ReviewSignalsResponse, ClientError> {
        let board = required_board(board)?;
        self.get(&signals_path(board, "signals/review", query))
    }

    pub fn get_signal(&self, signal_id: &str) -> Result<GetSignalResponse, ClientError> {
        let signal_id = required_signal_id(signal_id)?;
        self.get(&format!(
            "/api/v1/signals/{}",
            encode_path_segment(signal_id)
        ))
    }

    pub fn confirm_signals(
        &self,
        board: &str,
        request: &ReviewSignalsRequest,
    ) -> Result<ConfirmSignalsResponse, ClientError> {
        self.review_signal_action(board, "confirm", request)
    }

    pub fn reject_signals(
        &self,
        board: &str,
        request: &ReviewSignalsRequest,
    ) -> Result<RejectSignalsResponse, ClientError> {
        self.review_signal_action(board, "reject", request)
    }

    pub fn resolve_signals(
        &self,
        board: &str,
        request: &ReviewSignalsRequest,
    ) -> Result<ResolveSignalsResponse, ClientError> {
        self.review_signal_action(board, "resolve", request)
    }

    pub fn supersede_signals(
        &self,
        board: &str,
        request: &ReviewSignalsRequest,
    ) -> Result<SupersedeSignalsResponse, ClientError> {
        self.review_signal_action(board, "supersede", request)
    }

    fn review_signal_action(
        &self,
        board: &str,
        action: &str,
        request: &ReviewSignalsRequest,
    ) -> Result<kanban_protocol::DataEnvelope<Vec<SignalWire>>, ClientError> {
        let board = required_board(board)?;
        let mut request = request.clone();
        request.actor = Some(self.actor.clone());
        self.post(
            &format!(
                "/api/v1/boards/{}/signals/{action}",
                encode_path_segment(board)
            ),
            &request,
        )
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

fn signals_path(board: &str, suffix: &str, query: &SignalQuery) -> String {
    let mut pairs = Vec::new();
    for status in &query.status {
        pairs.push(format!("status={}", encode_path_segment(status)));
    }
    for kind in &query.kind {
        pairs.push(format!("kind={}", encode_path_segment(kind)));
    }
    if let Some(task_ref) = query.task_ref.as_deref().map(str::trim)
        && !task_ref.is_empty()
    {
        pairs.push(format!("task_ref={}", encode_path_segment(task_ref)));
    }
    if query.include_all {
        pairs.push("include_all=true".to_owned());
    }
    pairs.push(format!("limit={}", query.limit));
    format!(
        "/api/v1/boards/{}/{}?{}",
        encode_path_segment(board),
        suffix,
        pairs.join("&")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_paths_are_encoded_and_stable() {
        let path = signals_path(
            "team/one #",
            "signals/review",
            &SignalQuery {
                status: vec!["open".into(), "confirmed".into()],
                kind: vec!["failure/observed".into()],
                task_ref: Some("team/one#1".into()),
                include_all: true,
                limit: 12,
            },
        );
        assert_eq!(
            path,
            "/api/v1/boards/team%2Fone%20%23/signals/review?status=open&status=confirmed&kind=failure%2Fobserved&task_ref=team%2Fone%231&include_all=true&limit=12"
        );
    }

    #[test]
    fn invalid_signal_identifiers_are_rejected_before_http() {
        let client = KanbanClient::new(crate::DEFAULT_SERVER_URL, "test").unwrap();
        assert_eq!(
            client.get_signal("task#1").unwrap_err().code(),
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
                .unwrap_err()
                .code(),
            "invalid_input"
        );
    }
}
