import assert from "node:assert/strict";
import test from "node:test";
import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import { createKanbanRpc } from "../../dist/client.js";
import { createAtlasRpcRealtime } from "../../dist/atlas-client.js";
import { ChangeProtocolError } from "../../dist/changes.js";
import { ProtocolError } from "../../dist/model.js";
import { ProjectionStore } from "../../dist/reducer.js";
import {
  BoardFrameSchema, BoardService, GetBoardRequestSchema, GetBoardResponseSchema,
  RefreshReason, TaskCardSchema, TaskService, TaskStatus, UpdateTaskTitleRequestSchema,
  UpdateTaskTitleResponseSchema, WatchBoardRequestSchema, WatchChangesRequestSchema,
  WorkspaceChangeFrameSchema, WorkspaceService,
} from "../../dist/gen/kanban/framework/v1/board_pb.js";

const baseUrl = "http://127.0.0.1:8721";
const boardId = "b_generated";
const head = { boardId, epoch: "generated-epoch", scope: `board:${boardId}:cards:v1` };
const largeRevision = 9007199254740993n;
const card = {
  id: "t_generated", title: "生成契约", status: TaskStatus.TODO, priority: 1,
  position: -9223372036854775808n, seq: 18446744073709551615n, lockVersion: largeRevision,
};

function envelope(flags, bytes) {
  const header = Buffer.alloc(5);
  header[0] = flags;
  header.writeUInt32BE(bytes.length, 1);
  return Buffer.concat([header, bytes]);
}

function response(schema, messages, status = 0) {
  const frames = messages.map(message => envelope(0, toBinary(schema, create(schema, message))));
  frames.push(envelope(0x80, Buffer.from(`grpc-status: ${status}\r\n`)));
  return new Response(Buffer.concat(frames), { headers: { "content-type": "application/grpc-web+proto" } });
}

async function requestMessage(schema, request, init) {
  const wire = new Request(request, init);
  assert.equal(wire.method, "POST");
  assert.equal(wire.headers.get("content-type"), "application/grpc-web+proto");
  const bytes = new Uint8Array(await wire.arrayBuffer());
  assert.equal(bytes[0], 0);
  assert.equal(new DataView(bytes.buffer).getUint32(1), bytes.length - 5);
  return fromBinary(schema, bytes.subarray(5));
}

async function eventually(check) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (check()) return;
    await new Promise(resolve => setTimeout(resolve, 2));
  }
  assert.ok(check(), "客户端未在测试窗口内完成");
}

test("根 proto 生成三个命名 service，并保留 unary 和 server streaming 方法", () => {
  assert.equal(BoardService.method.getBoard.methodKind, "unary");
  assert.equal(BoardService.method.watchBoard.methodKind, "server_streaming");
  assert.equal(TaskService.method.updateTaskTitle.methodKind, "unary");
  assert.equal(WorkspaceService.method.watchChanges.methodKind, "server_streaming");
  assert.equal(WorkspaceService.typeName, "kanban.framework.v1.WorkspaceService");
});

test("实际 Protobuf-ES 编解码保留 int64、uint64 与 expected presence", () => {
  const decoded = fromBinary(TaskCardSchema, toBinary(TaskCardSchema, create(TaskCardSchema, card)));
  assert.equal(decoded.position, card.position);
  assert.equal(decoded.seq, card.seq);
  assert.equal(decoded.lockVersion, largeRevision);
  const present = fromBinary(UpdateTaskTitleRequestSchema, toBinary(UpdateTaskTitleRequestSchema,
    create(UpdateTaskTitleRequestSchema, { expected: { value: 0n } })));
  const absent = fromBinary(UpdateTaskTitleRequestSchema, toBinary(UpdateTaskTitleRequestSchema,
    create(UpdateTaskTitleRequestSchema)));
  assert.equal(present.expected?.value, 0n);
  assert.equal(absent.expected, undefined);
});

test("命名 unary 客户端使用生成的请求和响应，并保留零版本前置条件", async t => {
  const calls = [];
  t.mock.method(globalThis, "fetch", async (request, init) => {
    calls.push(request);
    assert.equal(init.redirect, "error");
    assert.equal(init.credentials, "omit");
    if (request.endsWith(".BoardService/GetBoard")) {
      assert.equal((await requestMessage(GetBoardRequestSchema, request, init)).boardId, boardId);
      return response(GetBoardResponseSchema, [{ boardId, cursor: { ...head, revision: largeRevision }, tasks: [card] }]);
    }
    const message = await requestMessage(UpdateTaskTitleRequestSchema, request, init);
    assert.equal(message.boardId, boardId);
    assert.equal(message.expected?.value, 0n);
    return response(UpdateTaskTitleResponseSchema, [{ task: { ...card, title: message.title } }]);
  });
  const rpc = createKanbanRpc(baseUrl);
  const board = await rpc.boards.getBoard({ boardId });
  const updated = await rpc.tasks.updateTaskTitle({ boardId, taskId: card.id, title: "更新后的标题", actor: "test", expected: { value: 0n } });
  assert.equal(board.cursor?.revision, largeRevision);
  assert.equal(board.tasks[0].seq, card.seq);
  assert.equal(updated.task?.title, "更新后的标题");
  assert.deepEqual(calls, [
    `${baseUrl}/kanban.framework.v1.BoardService/GetBoard`,
    `${baseUrl}/kanban.framework.v1.TaskService/UpdateTaskTitle`,
  ]);
});

test("WatchBoard 生成帧经客户端原子发布 snapshot 和 delta，重连携带已提交 cursor", async t => {
  const requests = [];
  const frames = [
    { ...head, body: { case: "snapshotBegin", value: { revision: largeRevision, count: 1 } } },
    { ...head, body: { case: "snapshotChunk", value: { index: 0, tasks: [card] } } },
    { ...head, body: { case: "snapshotCommit", value: { revision: largeRevision, count: 1, chunks: 1 } } },
    { ...head, body: { case: "delta", value: { baseRevision: largeRevision, revision: largeRevision + 1n,
      upserts: [{ ...card, title: "增量标题", lockVersion: largeRevision + 1n }], removedIds: [] } } },
  ];
  t.mock.method(globalThis, "fetch", async (request, init) => {
    requests.push(await requestMessage(WatchBoardRequestSchema, request, init));
    assert.equal(request, `${baseUrl}/kanban.framework.v1.BoardService/WatchBoard`);
    return requests.length === 1 ? response(BoardFrameSchema, frames) : response(BoardFrameSchema, [], 7);
  });
  const published = [];
  const store = new ProjectionStore(boardId, view => {
    published.push(view);
    if (view.cursor.revision === largeRevision + 1n) session.stop();
  });
  const rpc = createKanbanRpc(baseUrl);
  const session = rpc.watch(store);
  await session.start();
  assert.equal(published.length, 2);
  assert.equal(published[0].cards.get(card.id).title, card.title);
  assert.equal(published[1].cards.get(card.id).title, "增量标题");
  assert.equal(requests[0].protocolVersion, 1);
  assert.equal(requests[0].resume, undefined);
  await assert.rejects(session.start(), error => error.code === 7);
  assert.equal(requests[1].resume?.revision, largeRevision + 1n);
  assert.equal(requests[1].resume?.scope, head.scope);
});

test("未知 Protobuf oneof 或 TaskStatus 不会确认 snapshot cursor", async t => {
  for (const frames of [
    [head],
    [
      { ...head, body: { case: "snapshotBegin", value: { revision: 1n, count: 1 } } },
      { ...head, body: { case: "snapshotChunk", value: { index: 0, tasks: [{ ...card, status: 77 }] } } },
      { ...head, body: { case: "snapshotCommit", value: { revision: 1n, count: 1, chunks: 1 } } },
    ],
  ]) {
    t.mock.method(globalThis, "fetch", async () => response(BoardFrameSchema, frames));
    const published = [];
    const store = new ProjectionStore(boardId, view => published.push(view));
    const session = createKanbanRpc(baseUrl).watch(store, { terminal: error => error instanceof ProtocolError });
    await assert.rejects(session.start(), ProtocolError);
    assert.equal(store.resume(), undefined);
    assert.deepEqual(published, []);
  }
});

test("Atlas 生成客户端只为 attached 和 write_hint 刷新，固定同源 fetch 约束", async () => {
  const calls = [];
  const fetcher = async (request, init) => {
    calls.push(await requestMessage(WatchChangesRequestSchema, request, init));
    assert.equal(request, `${baseUrl}/rpc/kanban.framework.v1.WorkspaceService/WatchChanges`);
    assert.equal(init.mode, "same-origin");
    assert.equal(init.credentials, "same-origin");
    assert.equal(init.redirect, "error");
    assert.equal(init.cache, "no-store");
    return response(WorkspaceChangeFrameSchema, [
      { ...head, sequence: 1n, body: { case: "invalidated", value: { reason: RefreshReason.ATTACHED } } },
      { ...head, sequence: 1n, body: { case: "heartbeat", value: {} } },
      { ...head, sequence: 2n, body: { case: "invalidated", value: { reason: RefreshReason.WRITE_HINT } } },
    ]);
  };
  let refreshes = 0;
  const source = createAtlasRpcRealtime("/rpc/", `${baseUrl}/app/`, { maxFailures: 1 }, fetcher);
  const controller = source.create({ boardId, boardSelector: "generated", onRefresh: () => { refreshes += 1; }, onState() {} });
  controller.start();
  try {
    await eventually(() => controller.snapshot().state === "failed");
    assert.equal(calls.length, 1);
    assert.equal(calls[0].boardId, boardId);
    assert.equal(calls[0].protocolVersion, 1);
    assert.equal(refreshes, 2);
  } finally { controller.stop(); }
});

test("Atlas 拒绝生成代码保留的未知 reason 和缺失 oneof", async () => {
  for (const body of [undefined, { case: "invalidated", value: { reason: 77 } }]) {
    let failure;
    let refreshes = 0;
    const source = createAtlasRpcRealtime(baseUrl, `${baseUrl}/app/`, { maxFailures: 1 },
      async () => response(WorkspaceChangeFrameSchema, [{ ...head, sequence: 1n, ...(body ? { body } : {}) }]));
    const controller = source.create({ boardId, boardSelector: "generated", onRefresh: () => { refreshes += 1; },
      onState: (state, error) => { if (state === "failed") failure = error; } });
    controller.start();
    try {
      await eventually(() => controller.snapshot().state === "failed");
      assert.ok(failure instanceof ChangeProtocolError);
      assert.equal(refreshes, 0);
    } finally { controller.stop(); }
  }
});

test("Atlas 收到终止 gRPC 状态后停止，不再发请求或切换协议", async () => {
  let calls = 0;
  let failure;
  const source = createAtlasRpcRealtime(baseUrl, `${baseUrl}/app/`, { delay: async () => {} }, async () => {
    calls += 1;
    return response(WorkspaceChangeFrameSchema, [], 5);
  });
  const controller = source.create({ boardId, boardSelector: "generated", onRefresh: () => assert.fail("终止响应不能刷新"),
    onState: (state, error) => { if (state === "failed") failure = error; } });
  controller.start();
  try {
    await eventually(() => controller.snapshot().state === "failed");
    assert.equal(calls, 1);
    assert.equal(failure.code, 5);
  } finally { controller.stop(); }
});
