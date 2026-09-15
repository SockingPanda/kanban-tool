import { create } from '@bufbuild/protobuf'
import { describe, expect, test } from 'vitest'
import { QueryFrameSchema } from '../../generated/rpc/kanban/v1/query_pb'
import { encodeQueryDefinition } from './query-codec'
import { MAX_QUERY_RESULT_BYTES, QueryProjection } from './query-projection'
import { cursor, queryResultBytes, resultFrames } from './query-test-support'

const definition = encodeQueryDefinition({ method: 'ListBoards' }, 'q1')
const board = { id: 'b_a', slug: 'a', name: '甲', description: null, created_at: 1, updated_at: 1, archived_at: null }
const bytes = (name: string) => queryResultBytes(definition, { data: [{ ...board, name }] })
const acceptAll = async (projection: QueryProjection, frames: Awaited<ReturnType<typeof resultFrames>>) => {
  for (const frame of frames) await projection.accept(frame)
}

describe('完整 QueryResult 原子提交', () => {
  test('snapshot 与深层 Unicode delta 在 end 前保持旧 data 和 cursor，完整重建与具名 DTO 相同', async () => {
    const projection = new QueryProjection(definition)
    const first = await resultFrames('q1', bytes('甲'))
    await acceptAll(projection, first.slice(0, -1))
    expect(projection.committed).toBeNull()
    await projection.accept(first.at(-1)!)
    const previous = projection.committed!
    const delta = await resultFrames('q1', bytes('已同步💡'), cursor(2n), previous)
    await acceptAll(projection, delta.slice(0, -1))
    expect(projection.committed).toBe(previous)
    await projection.accept(delta.at(-1)!)
    expect(projection.committed?.encoded).toEqual(bytes('已同步💡'))
    expect(projection.committed?.response.payload).toEqual({ data: [{ ...board, name: '已同步💡' }] })
    expect(projection.committed?.cursor.revision).toBe(2n)
  })

  test('中断丢弃暂存，错误 base/epoch/scope 不改变最后已提交 cursor', async () => {
    const projection = new QueryProjection(definition)
    await acceptAll(projection, await resultFrames('q1', bytes('甲')))
    const previous = projection.committed!
    const delta = await resultFrames('q1', bytes('乙'), cursor(2n), previous)
    await acceptAll(projection, delta.slice(0, -1))
    projection.discard()
    await expect(projection.accept(delta.at(-1)!)).rejects.toThrow('完整')
    for (const invalidBase of [cursor(0n), cursor(1n, 'other'), cursor(1n, 'scope', 'restarted')]) {
      const bad = await resultFrames('q1', bytes('乙'), cursor(2n), { cursor: invalidBase, encoded: previous.encoded })
      await expect(projection.accept(bad[0])).rejects.toThrow('基线')
    }
    expect(projection.committed).toBe(previous)
  })

  test('损坏 hash、错 oneof、超预算和乱序块均拒绝提交，随后可恢复完整快照', async () => {
    const projection = new QueryProjection(definition)
    const corrupt = await resultFrames('q1', bytes('甲'))
    if (corrupt[0].body.case === 'begin') corrupt[0].body.value.resultSha256[0] ^= 1
    await acceptAll(projection, corrupt.slice(0, -1))
    await expect(projection.accept(corrupt.at(-1)!)).rejects.toThrow('SHA-256')
    projection.discard()
    const wrongDefinition = encodeQueryDefinition({ method: 'ListRuns', path: { task_id: 't_a' } }, 'q1')
    const wrong = await resultFrames('q1', queryResultBytes(wrongDefinition, { data: [] }))
    await acceptAll(projection, wrong.slice(0, -1))
    await expect(projection.accept(wrong.at(-1)!)).rejects.toThrow()
    projection.discard()
    const excessive = await resultFrames('q1', bytes('甲'))
    if (excessive[0].body.case === 'begin') excessive[0].body.value.resultSize = BigInt(MAX_QUERY_RESULT_BYTES + 1)
    await expect(projection.accept(excessive[0])).rejects.toThrow('预算')
    await projection.accept((await resultFrames('q1', bytes('甲')))[0])
    await expect(projection.accept(create(QueryFrameSchema, { clientQueryId: 'q1', body: { case: 'chunk', value: { index: 1, data: new Uint8Array([1]) } } }))).rejects.toThrow('块')
    projection.discard()
    await acceptAll(projection, await resultFrames('q1', bytes('恢复'), cursor(1n, 'scope', 'new-host')))
    expect(projection.committed?.response.payload).toEqual({ data: [{ ...board, name: '恢复' }] })
  })
})
