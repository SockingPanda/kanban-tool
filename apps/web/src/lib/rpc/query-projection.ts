import { fromBinary } from '@bufbuild/protobuf'
import { QueryResultSchema, type QueryBegin, type QueryCursor, type QueryDefinition, type QueryFrame } from '../../generated/rpc/kanban/v1/query_pb'
import { RpcTransportError, type RpcTransportResponse } from '../../application/data/rpc-transport'
import { decodeQueryResult } from './query-codec'

export const MAX_QUERY_RESULT_BYTES = 64 * 1024 * 1024
export const MAX_QUERY_CHUNK_BYTES = 60 * 1024
const MAX_QUERY_CHUNKS = 2048

export interface CommittedQuery {
  readonly cursor: QueryCursor
  readonly encoded: Uint8Array
  readonly response: RpcTransportResponse
}

interface PendingQuery {
  readonly begin: QueryBegin
  readonly patch: Uint8Array
  received: number
  chunks: number
}

function invalid(message: string): never {
  throw new RpcTransportError('invalid_bytes', `查询流无效：${message}`)
}

export function sameQueryCursor(left: QueryCursor | undefined, right: QueryCursor | undefined): boolean {
  return left !== undefined && right !== undefined
    && left.epoch === right.epoch && left.scope === right.scope && left.revision === right.revision
}

function validCursor(cursor: QueryCursor | undefined): cursor is QueryCursor {
  return cursor !== undefined && cursor.epoch.length > 0 && cursor.scope.length > 0
    && cursor.epoch.length <= 256 && cursor.scope.length <= 256 && cursor.revision > 0n
}

function boundedSize(value: bigint): number {
  if (value < 0n || value > BigInt(MAX_QUERY_RESULT_BYTES)) invalid('结果超过 64 MiB 预算。')
  return Number(value)
}

/** 只暂存一个 query 的帧；完整校验之前，已提交字节、数据与 cursor 均不改变。 */
export class QueryProjection {
  private pending: PendingQuery | null = null
  committed: CommittedQuery | null = null

  constructor(readonly definition: QueryDefinition) {}

  discard(): void { this.pending = null }

  get pendingBytes(): number { return this.pending?.patch.byteLength ?? 0 }

  async accept(frame: QueryFrame): Promise<CommittedQuery | null> {
    switch (frame.body.case) {
      case 'begin': {
        if (this.pending) invalid('前一结果尚未结束。')
        const begin = frame.body.value
        if (!validCursor(begin.cursor) || begin.resultSha256.byteLength !== 32) invalid('cursor 或 SHA-256 缺失。')
        const resultSize = boundedSize(begin.resultSize)
        const patchSize = boundedSize(begin.patchSize)
        const offset = boundedSize(begin.offset)
        const deleted = boundedSize(begin.deleteLength)
        if (begin.chunkCount > MAX_QUERY_CHUNKS || (patchSize === 0) !== (begin.chunkCount === 0)) invalid('块数不符合预算。')
        if (begin.snapshot) {
          if (offset !== 0 || deleted !== 0 || patchSize !== resultSize || begin.base) invalid('快照 splice 字段无效。')
        } else {
          const base = this.committed
          if (!base || !sameQueryCursor(base.cursor, begin.base)) invalid('delta 基线不匹配。')
          if (begin.cursor.epoch !== base.cursor.epoch || begin.cursor.scope !== base.cursor.scope || begin.cursor.revision <= base.cursor.revision) invalid('delta cursor 没有在相同作用域前进。')
          if (offset + deleted > base.encoded.byteLength || base.encoded.byteLength - deleted + patchSize !== resultSize) invalid('delta 超出基线边界。')
        }
        this.pending = { begin, patch: new Uint8Array(patchSize), received: 0, chunks: 0 }
        return null
      }
      case 'chunk': {
        const pending = this.pending
        const chunk = frame.body.value
        if (!pending || chunk.index !== pending.chunks || chunk.data.byteLength === 0
          || chunk.data.byteLength > MAX_QUERY_CHUNK_BYTES || pending.chunks >= pending.begin.chunkCount
          || pending.received + chunk.data.byteLength > pending.patch.byteLength) invalid('块顺序或长度无效。')
        pending.patch.set(chunk.data, pending.received)
        pending.received += chunk.data.byteLength
        pending.chunks += 1
        return null
      }
      case 'end': {
        const pending = this.pending
        if (!pending || !sameQueryCursor(pending.begin.cursor, frame.body.value.cursor)
          || pending.received !== pending.patch.byteLength || pending.chunks !== pending.begin.chunkCount) invalid('结果尚未完整接收。')
        const { begin, patch } = pending
        const encoded = begin.snapshot ? patch : new Uint8Array(Number(begin.resultSize))
        if (!begin.snapshot) {
          const previous = this.committed!.encoded
          const offset = Number(begin.offset)
          encoded.set(previous.subarray(0, offset))
          encoded.set(patch, offset)
          encoded.set(previous.subarray(offset + Number(begin.deleteLength)), offset + patch.byteLength)
        }
        const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', Uint8Array.from(encoded)))
        if (digest.some((byte, index) => byte !== begin.resultSha256[index])) invalid('SHA-256 校验失败。')
        const result = fromBinary(QueryResultSchema, encoded)
        const payload = decodeQueryResult(this.definition, result)
        // 网络切换可以在 digest await 期间丢弃暂存；旧结果不得提交。
        if (this.pending !== pending) return null
        const committed = { cursor: begin.cursor!, encoded, response: { payload, bytes: encoded.byteLength } }
        this.committed = committed
        this.pending = null
        return committed
      }
      case 'heartbeat': return null
      case 'ready': return null
      case 'failure': this.discard(); return null
      default: return invalid('缺少具名帧类型。')
    }
  }
}
