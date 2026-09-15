import { describe, expect, test, vi } from 'vitest'
import type { RpcTransport } from '../../application/data/rpc-transport'
import type { WebRuntimeConfig } from '../../lib/runtime'
import attachmentFixture from '../../lib/api/generated/fixtures/api-create-attachment-response.valid.json'
import { createAttachmentDownloadClient } from './attachment-download'

const runtime = { apiBaseUrl: '/__kb_api__', webBasePath: '/app/', actor: 'web-user', defaultBoard: 'default', serverVersion: '3.1.0', protocolVersion: 'v1', webBuildId: 'sha256:test' } satisfies WebRuntimeConfig
function setup(id = 'a_%中') {
  const content = new Uint8Array([0, 255, 13])
  const attachment = { ...attachmentFixture.data, id, task_id: 't_1', size_bytes: content.length, content_type: 'application/octet-stream', sha256: 'sha-fixture' }
  const call = vi.fn<RpcTransport['call']>(async () => ({ payload: { attachment, content }, bytes: content.length + 100 }))
  const client = createAttachmentDownloadClient(runtime, { transport: { call } })
  return { client, call, attachment, content }
}

describe('附件命名 RPC', () => {
  test.each(['a_%', 'a_%FF', 'a_中', 'a_%2f', 'a_%5c', 'a_%00', 'a_%2e'])('不经过 URL 编解码，保留 opaque ID %s', async id => {
    const { client, call, content } = setup(id)
    const signal = new AbortController().signal
    const result = await client.downloadAttachment('t_1', id, { signal })
    expect(call).toHaveBeenCalledWith({ method: 'DownloadAttachment', path: { task_id: 't_1', attachment_id: id }, signal })
    expect(result).toEqual({ content_type: 'application/octet-stream', attachment_id: id, sha256: 'sha-fixture', content })
    expect(result.content).toBe(content)
  })
  test.each([
    { id: 'a_wrong' }, { task_id: 't_other' }, { size_bytes: 10 },
  ])('拒绝不匹配的附件身份或长度 %j', async change => {
    const { client, call, attachment, content } = setup()
    call.mockResolvedValueOnce({ payload: { attachment: { ...attachment, ...change }, content }, bytes: 100 })
    await expect(client.downloadAttachment('t_1', attachment.id)).rejects.toMatchObject({ kind: 'invalid_bytes' })
  })
  test('拒绝缺失的内容和无效的 metadata', async () => {
    const { client, call, attachment } = setup()
    call.mockResolvedValueOnce({ payload: { attachment }, bytes: 1 })
    await expect(client.downloadAttachment('t_1', attachment.id)).rejects.toMatchObject({ kind: 'invalid_bytes' })
    call.mockResolvedValueOnce({ payload: { attachment: {}, content: new Uint8Array() }, bytes: 1 })
    await expect(client.downloadAttachment('t_1', attachment.id)).rejects.toMatchObject({ name: 'ContractValidationError' })
  })
  test('取消由调用者处理，不发起新的请求', async () => {
    const { client, call } = setup()
    const abort = new DOMException('cancelled', 'AbortError')
    call.mockRejectedValueOnce(abort)
    await expect(client.downloadAttachment('t_1', 'a_%中')).rejects.toBe(abort)
    expect(call).toHaveBeenCalledTimes(1)
  })
})
