//! 使用真实 stdio JSON-RPC、原生 gRPC 与隔离 canonical host 验证 MCP adapter。

#[path = "stdio_native/support.rs"]
mod support;

use kanban_protocol::{McpOperationClass, operation_catalog, project_mcp_policy};
use serde_json::{Value, json};
use support::{ACTOR, Host, Mcp, WAIT};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};

#[tokio::test]
async fn stdio_catalog_keeps_tool_contracts_and_rejects_host_admin() {
    let mut mcp = Mcp::start("http://127.0.0.1:1", "default").await;
    let response = mcp.request("tools/list", json!({})).await;
    let result = &response["result"];
    let tools = result["tools"].as_array().unwrap();
    let names: Vec<_> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    let projection = project_mcp_policy(operation_catalog()).unwrap();
    let mut expected: Vec<_> = projection
        .tool_bindings()
        .iter()
        .map(|tool| tool.tool_name)
        .collect();
    expected.sort_unstable();
    assert_eq!(names, expected);
    assert_eq!(names.len(), 105);
    for tool in tools {
        assert!(tool["description"].is_string());
        assert!(tool["inputSchema"].is_object());
    }
    for operation in projection.operations(McpOperationClass::HostAdmin) {
        assert!(
            projection
                .tool_bindings()
                .iter()
                .all(|tool| !tool.http_operations.contains(operation))
        );
    }
    // 迁移验收时传入由旧候选 binary 捕获的完整 tools/list；不复制第二份 catalog。
    if let Ok(path) = std::env::var("KANBAN_MCP_BASELINE_TOOLS") {
        let baseline: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(
            result, &baseline,
            "MCP tool description/schema/annotation 发生变化"
        );
    }
    if let Ok(path) = std::env::var("KANBAN_MCP_BASELINE_INITIALIZE") {
        let baseline: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(mcp.initialized, baseline, "MCP initialize 契约发生变化");
    }
    for name in [
        "backup",
        "checkpoint",
        "doctor",
        "export",
        "import",
        "vacuum",
        "database_replace",
    ] {
        assert!(!names.contains(&name));
        let error = mcp.call_error(name, json!({})).await;
        assert_eq!(error["code"], -32602, "{error}");
    }
    mcp.finish().await;
}

#[tokio::test]
async fn stdio_native_crud_preserves_selectors_patch_cas_idempotency_and_actor() {
    let host = Host::start().await;
    let mut mcp = Mcp::start(&host.url, "mcp-board").await;
    let board = mcp
        .call(
            "board_create",
            json!({"slug": "mcp-board", "name": "原生 MCP 看板"}),
        )
        .await;
    assert_eq!(board["data"]["slug"], "mcp-board");
    let boards = mcp.call("board_list", json!({})).await;
    assert!(
        boards["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == board["data"]["id"])
    );
    let input = json!({
        "title": "原生 async 任务", "description": "保留完整规格", "status": "todo",
        "task_id": "t_mcp_native_fixture", "idempotency_key": "mcp-create-once",
        "metadata": {"wide": 9007199254740993_u64}, "due_at": 9007199254740993_i64
    });
    let created = mcp.call("task_create", input.clone()).await;
    let retry = mcp.call("task_create", input).await;
    assert_eq!(created, retry);
    assert_eq!(created["data"]["metadata"]["wide"], 9007199254740993_u64);
    assert_eq!(created["data"]["due_at"], 9007199254740993_i64);
    let task = &created["data"];
    let selector = format!("#{}", task["seq"].as_i64().unwrap());
    let shown = mcp.call("task_show", json!({"task_ref": selector})).await;
    assert_eq!(shown["data"], *task);
    let updated = mcp.call("task_update", json!({
        "task_ref": selector, "description": null, "expected_lock_version": task["lock_version"]
    })).await;
    assert!(updated["data"]["description"].is_null());
    assert_eq!(updated["data"]["title"], task["title"]);
    assert_eq!(updated["data"]["due_at"], task["due_at"]);
    let stale = mcp.call_error("task_update", json!({
        "task_ref": selector, "title": "不应写入", "expected_lock_version": task["lock_version"]
    })).await;
    assert_eq!(stale["code"], -32602, "{stale}");
    assert!(
        stale["message"]
            .as_str()
            .unwrap()
            .starts_with("ClaimConflict:"),
        "{stale}"
    );
    let detail = mcp
        .call(
            "task_show",
            json!({"task_ref": selector, "include_details": true}),
        )
        .await;
    assert_eq!(detail["data"]["task"]["title"], task["title"]);
    let missing = mcp
        .call_error("task_show", json!({"task_ref": "#999999"}))
        .await;
    assert_eq!(missing["code"], -32602, "{missing}");
    let denied_actor = mcp
        .request(
            "tools/call",
            json!({"name": "task_create", "arguments": {"title": "拒绝注入", "actor": "spoofed"}}),
        )
        .await;
    assert!(denied_actor.get("error").is_none(), "{denied_actor}");
    assert_eq!(denied_actor["result"]["isError"], true, "{denied_actor}");
    assert!(
        denied_actor["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("unknown field `actor`"),
        "{denied_actor}"
    );
    let tasks = mcp.call("task_list", json!({})).await;
    assert_eq!(tasks["data"].as_array().unwrap().len(), 1);
    let comment_input = json!({"task_ref": selector, "body": "已核对原生调用", "idempotency_key": "mcp-comment-once"});
    let comment = mcp.call("comment_create", comment_input.clone()).await;
    let comment_retry = mcp.call("comment_create", comment_input).await;
    assert_eq!(comment, comment_retry);
    assert_eq!(comment["data"]["author"], ACTOR);
    let events = mcp.call("event_list", json!({"task_ref": selector})).await;
    let rows = events["data"].as_array().unwrap();
    assert!(rows.len() >= 3);
    assert!(
        rows.iter()
            .all(|event| event["task_id"] == task["id"] && event["actor"] == ACTOR),
        "{events}"
    );
    let board_events = mcp.call("event_list", json!({})).await;
    assert!(board_events["data"].as_array().unwrap().len() >= rows.len());
    mcp.finish().await;
    host.finish().await;
}

#[tokio::test]
async fn stdio_native_steps_attachments_and_claim_keep_business_semantics() {
    let host = Host::start().await;
    let mut mcp = Mcp::start(&host.url, "default").await;
    let created = mcp
        .call(
            "task_create",
            json!({"title": "执行任务", "description": "完整规格", "status": "todo"}),
        )
        .await;
    let selector = format!("default#{}", created["data"]["seq"].as_i64().unwrap());
    let step_input =
        json!({"task_ref": selector, "title": "验证主路径", "idempotency_key": "step-once"});
    let steps = mcp.call("step_create", step_input.clone()).await;
    assert_eq!(steps, mcp.call("step_create", step_input).await);
    assert_eq!(steps["data"]["steps"].as_array().unwrap().len(), 1);
    let done = mcp
        .call(
            "step_done",
            json!({"task_ref": selector, "step_ref": "S1", "note": "验证完成"}),
        )
        .await;
    assert_eq!(done["data"]["steps"][0]["status"], "done");
    let content = json!([0, 255, 228, 184, 173, 10]);
    let attachment = mcp
        .call(
            "attachment_create",
            json!({
                "task_ref": selector, "filename": "测试.bin", "content": content,
                "content_type": "application/octet-stream"
            }),
        )
        .await;
    let attachment_id = &attachment["data"]["id"];
    let downloaded = mcp
        .call(
            "attachment_download",
            json!({"task_ref": selector, "attachment_id": attachment_id}),
        )
        .await;
    assert_eq!(downloaded["content"], content);
    assert_eq!(downloaded["content_type"], "application/octet-stream");
    let listed = mcp
        .call("attachment_list", json!({"task_ref": selector}))
        .await;
    assert_eq!(listed["data"][0]["id"], *attachment_id);
    let removed = mcp
        .call(
            "attachment_remove",
            json!({"task_ref": selector, "attachment_id": attachment_id}),
        )
        .await;
    assert_eq!(removed["data"]["deleted"], true);
    let ready = mcp.call("task_show", json!({"task_ref": selector})).await;
    assert_eq!(ready["data"]["status"], "ready");
    let already_ready = mcp
        .call_error("task_promote", json!({"task_ref": selector}))
        .await;
    assert_eq!(already_ready["code"], -32602, "{already_ready}");
    assert!(
        already_ready["message"]
            .as_str()
            .unwrap()
            .starts_with("InvalidTransition:"),
        "{already_ready}"
    );
    let claim = mcp
        .call("task_claim", json!({"task_ref": selector, "ttl_ms": 60000}))
        .await;
    assert_eq!(claim["data"]["task"]["status"], "running");
    let token = &claim["data"]["claim_token"];
    assert!(!token.as_str().unwrap().is_empty());
    let duplicate = mcp
        .call_error("task_claim", json!({"task_ref": selector}))
        .await;
    assert_eq!(duplicate["code"], -32602, "{duplicate}");
    let wrong = mcp
        .call_error(
            "task_heartbeat",
            json!({"task_ref": selector, "claim_token": "wrong"}),
        )
        .await;
    assert_eq!(wrong["code"], -32602, "{wrong}");
    assert!(
        wrong["message"]
            .as_str()
            .unwrap()
            .starts_with("ClaimTokenMismatch:"),
        "{wrong}"
    );
    let heartbeat = mcp
        .call(
            "task_heartbeat",
            json!({"task_ref": selector, "claim_token": token, "ttl_ms": 60000}),
        )
        .await;
    assert_eq!(heartbeat["data"]["status"], "running");
    let released = mcp
        .call(
            "task_release",
            json!({"task_ref": selector, "claim_token": token}),
        )
        .await;
    assert_eq!(released["data"]["status"], "ready");
    let runs = mcp.call("run_list", json!({"task_ref": selector})).await;
    assert_eq!(runs["data"].as_array().unwrap().len(), 1);
    assert_eq!(runs["data"][0]["id"], claim["data"]["run"]["id"]);
    mcp.finish().await;
    host.finish().await;
}

async fn next_http2_frame(socket: &mut TcpStream) -> (u8, u32, Vec<u8>) {
    timeout(WAIT, async {
        let mut header = [0; 9];
        socket.read_exact(&mut header).await.unwrap();
        let len =
            usize::from(header[0]) << 16 | usize::from(header[1]) << 8 | usize::from(header[2]);
        assert!(len <= 16_384, "测试只接收小型 control/request frames");
        let stream = u32::from_be_bytes(header[5..9].try_into().unwrap()) & 0x7fff_ffff;
        let mut payload = vec![0; len];
        socket.read_exact(&mut payload).await.unwrap();
        (header[3], stream, payload)
    })
    .await
    .expect("原生 HTTP/2 frame 超时")
}

#[tokio::test]
async fn stdio_cancel_drops_the_inflight_native_grpc_request() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut mcp = Mcp::start(
        &format!("http://{}", listener.local_addr().unwrap()),
        "default",
    )
    .await;
    let id = mcp
        .send_request("tools/call", json!({"name": "board_list", "arguments": {}}))
        .await;
    let (mut socket, _) = timeout(WAIT, listener.accept()).await.unwrap().unwrap();
    let mut preface = [0; 24];
    timeout(WAIT, socket.read_exact(&mut preface))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&preface, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");
    // 建立 HTTP/2，保留请求未响应，直到 MCP 发出显式取消通知。
    socket
        .write_all(&[0, 0, 0, 4, 0, 0, 0, 0, 0])
        .await
        .unwrap();
    let stream_id = loop {
        let (kind, stream, _) = next_http2_frame(&mut socket).await;
        if kind == 1 {
            assert_ne!(stream, 0);
            break stream;
        }
    };
    mcp.notify(
        "notifications/cancelled",
        json!({"requestId": id, "reason": "停止当前工具调用"}),
    )
    .await;
    let ping = mcp.request("ping", json!({})).await;
    assert!(ping.get("error").is_none(), "{ping}");
    loop {
        let (kind, stream, payload) = next_http2_frame(&mut socket).await;
        if kind == 3 && stream == stream_id {
            assert_eq!(payload, [0, 0, 0, 8], "应发送 RST_STREAM CANCEL");
            break;
        }
    }
    let catalog = mcp.request("tools/list", json!({})).await;
    assert_eq!(catalog["result"]["tools"].as_array().unwrap().len(), 105);
    mcp.finish().await;
}
