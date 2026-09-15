import type { AttachmentDownloadClient,AttachmentDownloadDependencies } from '../../application/data/attachment-download'
import { RpcTransportError } from '../../application/data/rpc-transport'
import { parseApiCreateAttachmentResponse } from '../../lib/api/generated/contracts/api-create-attachment-response'
import { parseApiDownloadAttachmentPath } from '../../lib/api/generated/contracts/api-download-attachment-path'
import type { WebRuntimeConfig } from '../../lib/runtime'
import { createRpcTransport } from './rpc-transport'

export function createAttachmentDownloadClient(runtime: WebRuntimeConfig, dependencies: AttachmentDownloadDependencies = {}): AttachmentDownloadClient {
  const transport = dependencies.transport ?? createRpcTransport(runtime, dependencies)
  return {
    async downloadAttachment(taskId, attachmentId, options = {}) {
      const path = parseApiDownloadAttachmentPath({ task_id: taskId, attachment_id: attachmentId })
      const { payload } = await transport.call({ method: 'DownloadAttachment', path, ...(options.signal ? { signal: options.signal } : {}) })
      if (payload === null || typeof payload !== 'object' || !('attachment' in payload) || !('content' in payload) || !(payload.content instanceof Uint8Array)) {
        throw new RpcTransportError('invalid_bytes', '附件 RPC 响应缺少内容或元数据。')
      }
      const attachment = parseApiCreateAttachmentResponse({ data: payload.attachment }).data
      if (attachment.id !== attachmentId || attachment.task_id !== taskId || attachment.size_bytes !== payload.content.byteLength) {
        throw new RpcTransportError('invalid_bytes', '附件 RPC 响应与请求身份或大小不一致。')
      }
      if (payload.content.byteLength > 256 * 1024 * 1024) {
        throw new RpcTransportError('response_too_large', '附件超过 256 MiB 下载上限。')
      }
      return { content_type: attachment.content_type, attachment_id: attachment.id, sha256: attachment.sha256, content: payload.content }
    },
  }
}
