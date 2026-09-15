/** Atlas application 的实时会话入口。这里没有 HTTP、SSE、Protobuf 类型。 */
export type RealtimeState = "connecting" | "live" | "retrying" | "stopped" | "failed"
export interface BoardRealtimeContext {
  readonly boardId: string
  readonly boardSelector: string
  readonly onState: (state: RealtimeState, error?: unknown) => void
  /** 这是查询失效提示，不是审计事件，也不证明查询结果已应用。 */
  readonly onRefresh: () => void
}
export interface BoardRealtimeController {
  start(): void
  stop(): void
  retry(): void
  snapshot(): { readonly state: RealtimeState }
}
export interface BoardRealtimeSource {
  /** 同一 runtime 的会话不能混用不同 endpoint 或协议。 */
  readonly key: string
  create(context: BoardRealtimeContext): BoardRealtimeController
}
