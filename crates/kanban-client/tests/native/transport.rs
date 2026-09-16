use crate::common::{Http2Peer, WAIT, next_http2_frame, stalled_request};
use std::{sync::atomic::Ordering, time::Duration};
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn clones_share_one_lazy_http2_connection_and_channel_reconnects() {
    let proxy = Http2Peer::start().await;
    assert_eq!(proxy.accepted.load(Ordering::SeqCst), 0);
    let clone = proxy.client.clone();
    let (one, two, three) = tokio::join!(
        proxy.client.list_boards(false),
        clone.list_boards(true),
        clone.list_boards(false)
    );
    assert!(one.unwrap().is_empty());
    assert!(two.unwrap().is_empty());
    assert!(three.unwrap().is_empty());
    assert_eq!(proxy.accepted.load(Ordering::SeqCst), 1);
    proxy.disconnect().await;
    sleep(Duration::from_millis(20)).await;
    timeout(WAIT, async {
        loop {
            if proxy.client.list_boards(false).await.is_ok() {
                break;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(proxy.accepted.load(Ordering::SeqCst) >= 2);
    drop(proxy);
}

#[tokio::test]
async fn dropping_a_unary_future_sends_http2_cancel() {
    let (request, mut socket, stream_id) = stalled_request().await;
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    loop {
        let (kind, stream, payload) = next_http2_frame(&mut socket).await;
        if kind == 3 && stream == stream_id {
            assert_eq!(payload, [0, 0, 0, 8]);
            break;
        }
    }
}

#[tokio::test]
async fn unary_budget_cancels_a_stalled_rpc_after_thirty_seconds() {
    let (request, _socket, _) = stalled_request().await;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(31)).await;
    assert_eq!(
        request.await.unwrap().unwrap_err().code(),
        "server_unavailable"
    );
    tokio::time::resume();
}
