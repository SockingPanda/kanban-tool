import { fromBinary } from "@bufbuild/protobuf"
import { expect, type Page } from "@playwright/test"

import { QueryFrameSchema, WatchQueriesRequestSchema, type QueryCursor, type QueryDefinition, type QueryFrame } from "../src/generated/rpc/kanban/v1/query_pb"

export type QueryFrameObservation = {
  readonly connection: string
  readonly definition: QueryDefinition | undefined
  readonly frame: QueryFrame
}
type FrameAction = "pass" | "eof" | "complete" | "error"

/** 真实 Host 的逐帧故障注入：字节来自实际 Fetch，仅控制交付/中断，不生成业务结果。 */
export async function installQueryProbe(page: Page) {
  const requests: { connection: string; queries: QueryDefinition[]; atMs: number }[] = []
  const frames: QueryFrameObservation[] = []
  const actions: { connection: string; method: string | undefined; kind: string | undefined; action: FrameAction }[] = []
  const committed = new Map<string, { definition: QueryDefinition; cursor: QueryCursor }>()
  let handler: (observation: QueryFrameObservation) => FrameAction | Promise<FrameAction> = () => "pass"
  await page.exposeBinding("__kanbanProbeOpen", (_source, connection: string, bytes: number[]) => {
    const request = fromBinary(WatchQueriesRequestSchema, Uint8Array.from(bytes).subarray(5))
    requests.push({ connection, queries: request.queries, atMs: Date.now() })
  })
  await page.exposeBinding("__kanbanProbeFrame", async (_source, connection: string, bytes: number[]) => {
    const frame = fromBinary(QueryFrameSchema, Uint8Array.from(bytes).subarray(5))
    const definition = requests.find(request => request.connection === connection)?.queries.find(query => query.clientQueryId === frame.clientQueryId)
    const observation = { connection, definition, frame }
    frames.push(observation)
    const action = await handler(observation)
    actions.push({ connection, method: definition?.query.case, kind: frame.body.case, action })
    if (action === "pass" && definition && frame.body.case === "end" && frame.body.value.cursor) {
      committed.set(definition.clientQueryId, { definition, cursor: frame.body.value.cursor })
    }
    return action
  })
  await page.addInitScript(() => {
    type Bridge = { __kanbanProbeOpen(id: string, bytes: number[]): Promise<void>; __kanbanProbeFrame(id: string, bytes: number[]): Promise<"pass" | "eof" | "complete" | "error"> }
    const bridge = window as unknown as Bridge
    const nativeFetch = window.fetch.bind(window)
    const documentId = crypto.randomUUID()
    let serial = 0
    window.fetch = async (input, init) => {
      const url = new URL(typeof input === "string" ? input : input instanceof URL ? input.href : input.url, location.href)
      if (!url.pathname.endsWith("/kanban.v1.QueryService/WatchQueries")) return nativeFetch(input, init)
      const bytes = init?.body
      if (!(bytes instanceof Uint8Array)) throw new Error("Query probe requires binary request")
      const connection = documentId + ":" + ++serial
      const abort = new AbortController()
      const onAbort = () => abort.abort(init?.signal?.reason)
      init?.signal?.addEventListener("abort", onAbort, { once: true })
      if (init?.signal?.aborted) onAbort()
      await bridge.__kanbanProbeOpen(connection, Array.from(bytes))
      const response = await nativeFetch(input, { ...init, signal: abort.signal })
      if (!response.body || response.status !== 200) return response
      const reader = response.body.getReader()
      let finished = false
      const clean = () => { init?.signal?.removeEventListener("abort", onAbort) }
      const body = new ReadableStream<Uint8Array>({
        start(controller) {
          void (async () => {
            let pending: Uint8Array<ArrayBufferLike> = new Uint8Array()
            try {
              while (!finished) {
                const next = await reader.read()
                if (next.done) {
                  if (pending.length) controller.enqueue(pending)
                  controller.close(); finished = true; break
                }
                const joined = new Uint8Array(pending.length + next.value.length)
                joined.set(pending); joined.set(next.value, pending.length); pending = joined
                while (pending.length >= 5) {
                  const length = new DataView(pending.buffer, pending.byteOffset).getUint32(1) + 5
                  if (pending.length < length) break
                  const frame = pending.slice(0, length); pending = pending.slice(length)
                  const action = frame[0] === 128 ? "pass" : await bridge.__kanbanProbeFrame(connection, Array.from(frame))
                  if (finished || abort.signal.aborted) break
                  if (action === "pass") controller.enqueue(frame)
                  else {
                    finished = true
                    abort.abort()
                    void reader.cancel().catch(() => undefined)
                    if (action === "complete") {
                      const status = new TextEncoder().encode("grpc-status: 0\r\n")
                      const trailer = new Uint8Array(5 + status.length)
                      trailer[0] = 128
                      new DataView(trailer.buffer).setUint32(1, status.length)
                      trailer.set(status, 5)
                      controller.enqueue(trailer)
                      controller.close()
                    }
                    else if (action === "eof") controller.close()
                    else controller.error(new TypeError("故障注入：query stream disconnected"))
                    break
                  }
                }
              }
            } catch (error) { if (!finished) { finished = true; controller.error(error) } }
            finally { clean() }
          })()
        },
        async cancel() { finished = true; abort.abort(); clean(); await reader.cancel() },
      })
      const wrapped = new Response(body, { status: response.status, headers: response.headers })
      Object.defineProperty(wrapped, "url", { value: response.url })
      return wrapped
    }
  })
  return {
    requests, frames, actions,
    async settle() {
      await expect.poll(() => {
        const latest = requests.at(-1)
        return latest && Date.now() - latest.atMs > 150 && latest.queries.every(query => frames.some(item => item.connection === latest.connection && item.frame.clientQueryId === query.clientQueryId && item.frame.body.case === "ready"))
      }, { timeout: 10_000 }).toBe(true)
    },
    onFrame(next: typeof handler) { handler = next },
    committed(query: QueryDefinition["query"]["case"]) { return [...committed.values()].reverse().find(sample => sample.definition.query.case === query)?.cursor },
  }
}
