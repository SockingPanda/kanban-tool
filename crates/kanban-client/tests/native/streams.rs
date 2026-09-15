use crate::common::{Host, WAIT, input, task_request};
use kanban_client::{ClientError, EventStreamItem};
use kanban_protocol::{ListEventsQuery, StreamEventsQuery};
use serde_json::json;
use std::time::Duration;
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn stream_pages_resume_from_newer_cursor_and_keep_scope_after_cancelled_read() {
    let host = Host::start().await;
    let client = &host.client;
    let task = client
        .create_task("default", task_request("t_stream"))
        .await
        .unwrap();
    let after = client
        .list_events(&ListEventsQuery {
            board: "default".into(),
            task_id: Some(task.id.clone()),
            after: 0,
            limit: 100,
        })
        .await
        .unwrap()
        .meta
        .next_after;
    for i in 0..5 {
        client
            .create_comment(&task.id, &input(json!({"body":format!("记录 {i}")})))
            .await
            .unwrap();
    }
    let query = StreamEventsQuery {
        board: "default".into(),
        task_id: Some(task.id.clone()),
        after: 0,
        limit: 2,
    };
    let mut stream = client.open_event_stream(&query, Some(after)).await.unwrap();
    let mut ids = Vec::new();
    for _ in 0..5 {
        let EventStreamItem::Business(event) =
            timeout(WAIT, stream.next_item()).await.unwrap().unwrap()
        else {
            panic!("应读取领域事件")
        };
        assert_eq!(event.kind, "task.comment.created");
        assert_eq!(event.task_id.as_deref(), Some(task.id.as_str()));
        ids.push(event.id);
    }
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(ids[0] > after);
    assert!(
        timeout(Duration::from_millis(40), stream.next_item())
            .await
            .is_err()
    );
    client
        .create_comment(&task.id, &input(json!({"body":"取消读取之后的事件"})))
        .await
        .unwrap();
    let EventStreamItem::Business(event) =
        timeout(WAIT, stream.next_item()).await.unwrap().unwrap()
    else {
        panic!("丢失取消后的事件")
    };
    assert!(event.id > *ids.last().unwrap());
    drop(stream);
    let mut resumed = client
        .open_event_stream(
            &StreamEventsQuery {
                after: event.id,
                ..query
            },
            Some(after),
        )
        .await
        .unwrap();
    assert!(
        timeout(Duration::from_millis(40), resumed.next_item())
            .await
            .is_err()
    );
    drop(resumed);
    host.finish().await;
}

#[tokio::test]
async fn dropping_streams_releases_all_host_subscription_slots() {
    let host = Host::start().await;
    let query = StreamEventsQuery {
        board: "default".into(),
        task_id: None,
        after: 0,
        limit: 10,
    };
    let mut streams = Vec::new();
    for _ in 0..16 {
        streams.push(host.client.open_event_stream(&query, None).await.unwrap());
    }
    assert!(host.client.open_event_stream(&query, None).await.is_err());
    drop(streams);
    let stream = timeout(WAIT, async {
        loop {
            match host.client.open_event_stream(&query, None).await {
                Ok(stream) => break stream,
                Err(_) => sleep(Duration::from_millis(10)).await,
            }
        }
    })
    .await
    .unwrap();
    drop(stream);
    host.finish().await;
}

#[tokio::test]
async fn heartbeat_preserves_cursor_and_host_shutdown_closes_stream() {
    let host = Host::start().await;
    let after = host
        .client
        .list_events(&ListEventsQuery {
            board: "default".into(),
            task_id: None,
            after: 0,
            limit: 1000,
        })
        .await
        .unwrap()
        .meta
        .next_after;
    let mut stream = host
        .client
        .open_event_stream(
            &StreamEventsQuery {
                board: "default".into(),
                task_id: None,
                after,
                limit: 10,
            },
            None,
        )
        .await
        .unwrap();
    assert!(matches!(
        timeout(Duration::from_secs(20), stream.next_item())
            .await
            .unwrap()
            .unwrap(),
        EventStreamItem::Heartbeat(_)
    ));
    host.finish().await;
    assert!(matches!(
        timeout(WAIT, stream.next_item()).await.unwrap(),
        Err(ClientError::StreamClosed)
    ));
}
