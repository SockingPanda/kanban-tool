import { create, toBinary, fromJsonString } from '@bufbuild/protobuf'
import { QueryCursorSchema, QueryFrameSchema, QueryResultSchema, type QueryCursor, type QueryDefinition, type QueryFrame } from '../../generated/rpc/kanban/v1/query_pb'
import * as codec from './codec.generated'
import { encodeJson } from './value-codec'
import { stringifyJson } from '../lossless-json'
import { FileListOutputSchema } from '../../generated/rpc/kanban/extensions/v1/workspace_pb'

/** 测试从真实具名 DTO 编码生成 wire，避免手写字节绕过 codec。 */
export function queryResultBytes(definition: QueryDefinition, payload: unknown): Uint8Array {
  let result: ReturnType<typeof create<typeof QueryResultSchema>>['result']
  switch (definition.query.case) {
    case 'getObject': case 'listObjects': case 'getObjectCatalog': case 'getObjectOverview': case 'getWorkflowClosure': case 'getObjectReferences': case 'getObjectHistory': case 'getObjectSnapshots': case 'diagnoseObjects': result = { case: definition.query.case, value: { $typeName: 'kanban.extensions.v1.ObjectDocument', data: encodeJson(payload) } }; break
    case 'listObjectFiles': result = { case: 'listObjectFiles', value: fromJsonString(FileListOutputSchema, stringifyJson(payload)) }; break
    case 'listBoards': result = { case: 'listBoards', value: codec.encodeListBoardsResponse(payload) }; break
    case 'listBoardColumns': result = { case: 'listBoardColumns', value: codec.encodeListBoardColumnsResponse(payload) }; break
    case 'listTasks': result = { case: 'listTasks', value: codec.encodeListTasksResponse(payload) }; break
    case 'listTasksByStatus': result = { case: 'listTasksByStatus', value: codec.encodeListTasksByStatusResponse(payload) }; break
    case 'getTask': result = { case: 'getTask', value: codec.encodeGetTaskResponse(payload) }; break
    case 'listTaskLabels': result = { case: 'listTaskLabels', value: codec.encodeListTaskLabelsResponse(payload) }; break
    case 'listDependencies': result = { case: 'listDependencies', value: codec.encodeListDependenciesResponse(payload) }; break
    case 'listSteps': result = { case: 'listSteps', value: codec.encodeListStepsResponse(payload) }; break
    case 'listComments': result = { case: 'listComments', value: codec.encodeListCommentsResponse(payload) }; break
    case 'listAttachments': result = { case: 'listAttachments', value: codec.encodeListAttachmentsResponse(payload) }; break
    case 'taskNeighborhood': result = { case: 'taskNeighborhood', value: codec.encodeTaskNeighborhoodResponse(payload) }; break
    case 'boardTaskMap': result = { case: 'boardTaskMap', value: codec.encodeBoardTaskMapResponse(payload) }; break
    case 'listRuns': result = { case: 'listRuns', value: codec.encodeListRunsResponse(payload) }; break
    case 'getRunLog': result = { case: 'getRunLog', value: codec.encodeGetRunLogResponse(payload) }; break
    case 'listEvents': result = { case: 'listEvents', value: codec.encodeListEventsResponse(payload) }; break
    case 'recentEvents': result = { case: 'recentEvents', value: codec.encodeListEventsResponse(payload) }; break
    case 'getStats': result = { case: 'getStats', value: codec.encodeGetStatsResponse(payload) }; break
    default: throw new Error(`缺少测试回复：${definition.query.case}`)
  }
  return toBinary(QueryResultSchema, create(QueryResultSchema, { result }))
}

export function cursor(revision = 1n, scope = 'scope', epoch = 'epoch'): QueryCursor {
  return create(QueryCursorSchema, { epoch, scope, revision })
}

export async function resultFrames(id: string, encoded: Uint8Array, next = cursor(), previous?: { readonly cursor: QueryCursor; readonly encoded: Uint8Array }): Promise<QueryFrame[]> {
  let offset = 0
  let suffix = 0
  if (previous) {
    while (offset < Math.min(previous.encoded.length, encoded.length) && previous.encoded[offset] === encoded[offset]) offset += 1
    while (suffix < Math.min(previous.encoded.length, encoded.length) - offset && previous.encoded[previous.encoded.length - suffix - 1] === encoded[encoded.length - suffix - 1]) suffix += 1
  }
  const patch = encoded.subarray(offset, encoded.length - suffix)
  const chunks = []
  for (let index = 0; index < patch.length; index += 60 * 1024) chunks.push(patch.subarray(index, index + 60 * 1024))
  const sha = new Uint8Array(await crypto.subtle.digest('SHA-256', Uint8Array.from(encoded)))
  return [
    create(QueryFrameSchema, { clientQueryId: id, body: { case: 'begin', value: {
      cursor: next, base: previous?.cursor, snapshot: !previous,
      resultSize: BigInt(encoded.length), resultSha256: sha, offset: BigInt(offset),
      deleteLength: previous ? BigInt(previous.encoded.length - offset - suffix) : 0n,
      patchSize: BigInt(patch.length), chunkCount: chunks.length,
    } } }),
    ...chunks.map((data, index) => create(QueryFrameSchema, { clientQueryId: id, body: { case: 'chunk', value: { index, data } } })),
    create(QueryFrameSchema, { clientQueryId: id, body: { case: 'end', value: { cursor: next } } }),
  ]
}

export function readyFrame(id: string, at = cursor()): QueryFrame {
  return create(QueryFrameSchema, { clientQueryId: id, body: { case: 'ready', value: { cursor: at } } })
}

export class QueryFrameQueue {
  private frames: QueryFrame[] = []
  private wake: (() => void) | null = null
  private failure: Error | null = null
  push(...frames: QueryFrame[]): void { this.frames.push(...frames); this.wake?.() }
  fail(error: Error): void { this.failure = error; this.wake?.() }
  async *stream(signal: AbortSignal): AsyncIterable<QueryFrame> {
    const wake = () => this.wake?.()
    signal.addEventListener('abort', wake, { once: true })
    try {
      while (!signal.aborted) {
        const frame = this.frames.shift()
        if (frame) yield frame
        else if (this.failure) throw this.failure
        else await new Promise<void>(resolve => { this.wake = resolve })
      }
    } finally { signal.removeEventListener('abort', wake) }
  }
}

export function grpcWebFrame(encoded: Uint8Array): Uint8Array {
  const frame = new Uint8Array(encoded.length + 5)
  new DataView(frame.buffer).setUint32(1, encoded.length)
  frame.set(encoded, 5)
  return frame
}
