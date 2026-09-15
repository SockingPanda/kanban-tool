import type { BoardRealtimeSource } from "./source";
import type { CanonicalBoardId, SyncTelemetryEntry } from "../sync/contracts";

export interface RealtimeBindingOptions {
  readonly boardId: CanonicalBoardId;
  readonly boardSelector: string;
  readonly record: (entry: SyncTelemetryEntry) => void;
}
/** 控制消息只通知查询失效。cursor=0 是旧 telemetry 接口的占位，不进入事件去重或续读。 */
export function bindBoardRealtime(source: BoardRealtimeSource, options: RealtimeBindingOptions) {
  let active = false;
  const record = (type: string): void => {
    if (active) options.record({ type, boardId: options.boardId, cursor: 0,
      details: { controlOnly: true, realtimeSource: source.key } });
  };
  const controller = source.create({
    boardId: options.boardId,
    boardSelector: options.boardSelector,
    onRefresh: () => record("rpc-refresh-required"),
    onState: (state) => {
      switch (state) {
        case "live": record("connection-live"); break;
        case "retrying": record("transport-failure"); break;
        case "failed": record("circuit-open"); break;
        case "connecting": record("rpc-connecting"); break;
        case "stopped": break;
      }
    },
  });
  return {
    start(): void { active = true; controller.start(); },
    stop(): void { active = false; controller.stop(); },
    retry(): void { active = true; controller.retry(); },
    snapshot() {
      const state = controller.snapshot().state;
      return { state: state === "failed" ? "circuit-open" as const
        : state === "retrying" ? "recovering" as const : state };
    },
  };
}
