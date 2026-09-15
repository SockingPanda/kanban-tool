use kanban_rpc_proto::{PROTOCOL_VERSION, v1::*};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let channel = tonic::transport::Endpoint::from_static("http://127.0.0.1:50051").connect().await?;
    let mut boards = board_service_client::BoardServiceClient::new(channel.clone());
    let mut tasks = task_service_client::TaskServiceClient::new(channel);
    let snapshot = boards.get_board(GetBoardRequest { board_id: "b_default".into() }).await?.into_inner();
    let mut stream = boards.watch_board(WatchBoardRequest { board_id: "b_default".into(),
        resume: snapshot.cursor, protocol_version: PROTOCOL_VERSION }).await?.into_inner();
    let task = snapshot.tasks.iter().find(|t| t.id == "t_demo").ok_or("demo task missing")?;
    tasks.update_task_title(UpdateTaskTitleRequest { board_id: "b_default".into(), task_id: task.id.clone(),
        title: "原生 gRPC 修改成功".into(), actor: "native-example".into(), expected: Some(ExpectedVersion { value: task.lock_version }) }).await?;
    while let Some(frame) = stream.message().await? {
        println!("{frame:?}");
        if matches!(frame.body, Some(board_frame::Body::Delta(_))) { break; }
    }
    Ok(())
}
