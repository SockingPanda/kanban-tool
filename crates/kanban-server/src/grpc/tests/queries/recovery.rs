//! G07 的内存 Hub 恢复规则在真实 HTTP/2 与 binary gRPC-Web 上具有相同完整结果。
use super::common::{self, Projection, Transport};
use super::{Body, Host, Query, pb};

#[tokio::test]
async fn native_and_binary_web_resume_exact_delta_after_disconnect() {
    let host = Host::start().await;
    let mut client = common::client(&host).await;
    let task = common::task(&mut client, &host.board, "跨传输恢复").await;
    let query = common::comments_query(&task);
    let mut native = Transport::Native.watch(&host, query.clone(), None).await;
    let mut original = Projection::default();
    original.transaction(&mut native).await;
    let mut web = Transport::Web.watch(&host, query.clone(), None).await;
    let mut recovered = Projection::default();
    recovered.transaction(&mut web).await;
    assert_eq!(recovered.cursor, original.cursor);
    assert_eq!(
        recovered.bytes,
        common::authoritative(&mut client, &query).await
    );
    drop(web);

    common::comment(&mut client, &task, "断线期间的完整评论字段 🦓").await;
    original.transaction(&mut native).await;
    let base = recovered.cursor.clone();
    let mut web = Transport::Web
        .watch(&host, query.clone(), base.clone())
        .await;
    let resumed = recovered.transaction(&mut web).await;
    assert!(!resumed.snapshot);
    assert_eq!(resumed.base, base);
    assert_eq!(recovered.cursor, original.cursor);
    assert_eq!(
        recovered.bytes,
        common::authoritative(&mut client, &query).await
    );
    drop(native);

    let mut native_projection = recovered.clone();
    common::comment(&mut client, &task, "反向恢复：Web 保活、native 续传").await;
    recovered.transaction(&mut web).await;
    let mut native = Transport::Native
        .watch(&host, query.clone(), native_projection.cursor.clone())
        .await;
    assert!(!native_projection.transaction(&mut native).await.snapshot);
    assert_eq!(native_projection.cursor, recovered.cursor);
    assert_eq!(
        native_projection.bytes,
        common::authoritative(&mut client, &query).await
    );
    println!(
        "G08_QUERY_RESUME transports=native,grpc-web exact_base_delta=true complete_bytes={}",
        native_projection.bytes.len()
    );
    drop((native, web));
    common::idle(&host).await;
    host.finish().await;
}

#[tokio::test]
async fn lagging_native_and_web_resume_snapshot_after_real_history_eviction() {
    let host = Host::start().await;
    let mut client = common::client(&host).await;
    let task = common::task(&mut client, &host.board, "历史窗口 0").await;
    let query = common::task_query(&task);
    let mut anchor = Transport::Native.watch(&host, query.clone(), None).await;
    let mut current = Projection::default();
    current.transaction(&mut anchor).await;
    let mut native = Transport::Native.watch(&host, query.clone(), None).await;
    let mut web = Transport::Web.watch(&host, query.clone(), None).await;
    let mut old_native = Projection::default();
    let mut old_web = Projection::default();
    old_native.transaction(&mut native).await;
    old_web.transaction(&mut web).await;
    let oldest = current.cursor.clone().unwrap();
    let writes = kanban_live_core::query::QueryLimits::default().history_batches + 2;
    // 不再消费两个旧连接；另一个真实消费者逐次确认发布，避免 hint 合并减少实际历史条数。
    for index in 1..=writes {
        common::title(&mut client, &task, &format!("历史窗口 {index}")).await;
        let begin = current.transaction(&mut anchor).await;
        assert!(!begin.snapshot);
        assert_eq!(
            current.cursor.as_ref().unwrap().revision,
            oldest.revision + index as u64
        );
    }
    drop((native, web));
    for (transport, projection) in [
        (Transport::Native, &mut old_native),
        (Transport::Web, &mut old_web),
    ] {
        let mut resumed = transport
            .watch(&host, query.clone(), projection.cursor.clone())
            .await;
        let begin = projection.transaction(&mut resumed).await;
        assert!(begin.snapshot, "{transport:?} 必须重置已淘汰的 cursor");
        assert!(begin.base.is_none());
        assert_eq!(projection.cursor, current.cursor);
        assert_eq!(
            projection.bytes,
            common::authoritative(&mut client, &query).await
        );
        drop(resumed);
    }
    println!(
        "G08_QUERY_HISTORY transports=native,grpc-web committed_mutations={writes} reset_snapshot=true oldest_revision={} latest_revision={}",
        oldest.revision,
        current.cursor.unwrap().revision
    );
    drop(anchor);
    common::idle(&host).await;
    host.finish().await;
}

#[tokio::test]
async fn interrupted_wire_snapshot_keeps_cursor_until_complete_cross_transport_resume() {
    let host = Host::start().await;
    let mut client = common::client(&host).await;
    let task = common::task(&mut client, &host.board, "分块取消").await;
    let query = common::comments_query(&task);
    let mut anchor = Transport::Native.watch(&host, query.clone(), None).await;
    let mut initial = Projection::default();
    initial.transaction(&mut anchor).await;
    common::comment(&mut client, &task, &"完整投影🦓".repeat(32 * 1024)).await;
    let mut latest = initial.clone();
    latest.transaction(&mut anchor).await;
    let expected = common::authoritative(&mut client, &query).await;

    for (interrupted, resumed) in [
        (Transport::Native, Transport::Web),
        (Transport::Web, Transport::Native),
    ] {
        let mut projection = initial.clone();
        let mut wire = interrupted.watch(&host, query.clone(), None).await;
        let begin = wire.next().await;
        assert!(
            matches!(&begin.body, Some(Body::Begin(begin)) if begin.snapshot && begin.chunk_count > 1)
        );
        projection.apply(begin);
        let chunk = wire.next().await;
        assert!(matches!(&chunk.body, Some(Body::Chunk(chunk)) if chunk.index == 0));
        projection.apply(chunk);
        drop(wire);
        assert_eq!(projection.cursor, initial.cursor);
        assert_eq!(projection.bytes, initial.bytes);
        projection.discard_partial();

        let mut wire = resumed
            .watch(&host, query.clone(), projection.cursor.clone())
            .await;
        let begin = projection.transaction(&mut wire).await;
        assert!(!begin.snapshot);
        assert_eq!(begin.base, initial.cursor);
        assert_eq!(projection.cursor, latest.cursor);
        assert_eq!(projection.bytes, expected);
        drop(wire);
    }
    println!(
        "G08_QUERY_INTERRUPTED transports=native,grpc-web old_cursor_preserved=true result_bytes={} complete_sha_verified=true",
        expected.len()
    );
    drop(anchor);
    common::idle(&host).await;
    host.finish().await;
}

#[tokio::test]
async fn recycled_hub_changed_query_scope_and_new_host_epoch_reset_wire_cursor() {
    let host = Host::start().await;
    let mut client = common::client(&host).await;
    let first = common::task(&mut client, &host.board, "scope A").await;
    let second = common::task(&mut client, &host.board, "scope B").await;
    let query_a = common::task_query(&first);
    let query_b = common::task_query(&second);
    let mut wire = Transport::Native.watch(&host, query_a.clone(), None).await;
    let mut old = Projection::default();
    old.transaction(&mut wire).await;
    let original_cursor = old.cursor.clone().unwrap();
    drop(wire);
    common::idle(&host).await;

    common::title(&mut client, &first, "没有订阅时的权威更改").await;
    let mut recycled = Transport::Web
        .watch(&host, query_a.clone(), old.cursor.clone())
        .await;
    assert!(old.transaction(&mut recycled).await.snapshot);
    let recycled_cursor = old.cursor.clone().unwrap();
    assert_eq!(recycled_cursor.epoch, original_cursor.epoch);
    assert_ne!(recycled_cursor.scope, original_cursor.scope);
    assert_eq!(
        old.bytes,
        common::authoritative(&mut client, &query_a).await
    );

    let mut changed = Transport::Native
        .watch(&host, query_b.clone(), old.cursor.clone())
        .await;
    assert!(old.transaction(&mut changed).await.snapshot);
    assert_ne!(old.cursor.as_ref().unwrap().scope, recycled_cursor.scope);
    assert_eq!(
        old.bytes,
        common::authoritative(&mut client, &query_b).await
    );
    drop((recycled, changed));
    common::idle(&host).await;

    let boards = Query::ListBoards(pb::ListBoardsRequest::default());
    let mut wire = Transport::Native.watch(&host, boards.clone(), None).await;
    let mut previous_host = Projection::default();
    previous_host.transaction(&mut wire).await;
    let epoch = previous_host.cursor.as_ref().unwrap().epoch.clone();
    drop(wire);
    common::idle(&host).await;
    host.finish().await;

    let replacement = Host::start().await;
    let mut client = common::client(&replacement).await;
    let mut wire = Transport::Web
        .watch(&replacement, boards.clone(), previous_host.cursor.clone())
        .await;
    assert!(previous_host.transaction(&mut wire).await.snapshot);
    assert_ne!(previous_host.cursor.as_ref().unwrap().epoch, epoch);
    assert_eq!(
        previous_host.bytes,
        common::authoritative(&mut client, &boards).await
    );
    println!(
        "G08_QUERY_RESET recycled_scope=true changed_query_scope=true closed_host_epoch=true authoritative_bytes=true"
    );
    drop(wire);
    common::idle(&replacement).await;
    replacement.finish().await;
}
