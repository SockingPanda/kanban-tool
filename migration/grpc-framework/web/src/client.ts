import { createClient, ConnectError, Code } from "@connectrpc/connect"
import { createGrpcWebTransport } from "@connectrpc/connect-web"
import { BoardService, TaskService, type BoardFrame, type TaskCard } from "./gen/kanban/framework/v1/board_pb.js"
import { ProtocolError, type Card, type Frame } from "./model.js"
import { ProjectionStore } from "./reducer.js"
import { WatchSession, type WatchOptions } from "./watch.js"

function card(value: TaskCard): Card {
  return { id: value.id, title: value.title, status: value.status, priority: value.priority,
    position: value.position, seq: value.seq, lockVersion: value.lockVersion }
}
function frame(value: BoardFrame): Frame {
  const head = { boardId: value.boardId, epoch: value.epoch, scope: value.scope }
  const body = value.body
  switch (body.case) {
    case "snapshotBegin": return { ...head, body: { kind: "begin", revision: body.value.revision, count: body.value.count } }
    case "snapshotChunk": return { ...head, body: { kind: "chunk", index: body.value.index, tasks: body.value.tasks.map(card) } }
    case "snapshotCommit": return { ...head, body: { kind: "commit", revision: body.value.revision, count: body.value.count, chunks: body.value.chunks } }
    case "delta": return { ...head, body: { kind: "delta", base: body.value.baseRevision, revision: body.value.revision, upserts: body.value.upserts.map(card), removed: body.value.removedIds } }
    case "heartbeat": return { ...head, body: { kind: "heartbeat", serverRevision: body.value.serverRevision } }
    case "reset": return { ...head, body: { kind: "reset", reason: body.value.reason } }
    default: throw new ProtocolError("无法识别的 BoardFrame oneof")
  }
}
export function createKanbanRpc(baseUrl: string) {
  const url = new URL(baseUrl, globalThis.location?.origin)
  if (!["127.0.0.1", "localhost", "[::1]"].includes(url.hostname) || !["http:", "https:"].includes(url.protocol)
    || url.username !== "" || url.password !== "" || url.search !== "" || url.hash !== "") throw new Error("只允许明确的本地 RPC endpoint")
  // 使用 Connect 的 gRPC-Web transport，不启用 Connect wire protocol 或 REST fallback。
  const transport = createGrpcWebTransport({ baseUrl: url.href, useBinaryFormat: true,
    fetch: (input, init) => fetch(input, { ...init, redirect: "error", credentials: "omit", cache: "no-store" }) })
  const boards = createClient(BoardService, transport)
  const tasks = createClient(TaskService, transport)
  return {
    boards, tasks,
    watch(store: ProjectionStore, options: WatchOptions = {}) {
      return new WatchSession(store, async function* ({ boardId, resume, signal }) {
        for await (const value of boards.watchBoard({ boardId, protocolVersion: 1, ...(resume ? { resume } : {}) }, { signal })) yield frame(value)
      }, { ...options, terminal: (error) => options.terminal?.(error) === true || error instanceof ConnectError &&
        [Code.PermissionDenied, Code.Unauthenticated, Code.InvalidArgument, Code.FailedPrecondition, Code.Unimplemented].includes(error.code) })
    },
  }
}
