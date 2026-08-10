import type { WebRuntimeConfig } from "../runtime"
import { parseApiDownloadAttachmentHeaders } from "./generated/contracts/api-download-attachment-headers"
import { parseApiDownloadAttachmentPath } from "./generated/contracts/api-download-attachment-path"
import {
  createHttpTransport,
  type HttpTransport,
  type HttpTransportBytesResponse,
  type HttpTransportOptions,
} from "./http-transport"

export interface AttachmentDownloadOptions {
  readonly signal?: AbortSignal
}

export interface DownloadedAttachment {
  readonly content_type: string | null
  readonly attachment_id: string | null
  readonly sha256: string | null
  readonly content: Uint8Array
}

export interface AttachmentDownloadDependencies extends HttpTransportOptions {
  readonly transport?: Pick<HttpTransport, "requestBytes">
}

export interface AttachmentDownloadClient {
  downloadAttachment(
    taskId: string,
    attachmentId: string,
    options?: AttachmentDownloadOptions,
  ): Promise<DownloadedAttachment>
}

function encodedSegment(value: string): string {
  return encodeURIComponent(value)
}

function mapDownloadedAttachment(response: HttpTransportBytesResponse): DownloadedAttachment {
  return {
    content_type: response.contentType,
    attachment_id: response.attachmentId,
    sha256: response.sha256,
    content: response.bytes,
  }
}

export function createAttachmentDownloadClient(
  runtime: WebRuntimeConfig,
  dependencies: AttachmentDownloadDependencies = {},
): AttachmentDownloadClient {
  const transport = dependencies.transport ?? createHttpTransport(runtime, dependencies)

  return {
    async downloadAttachment(taskId, attachmentId, options = {}) {
      const path = parseApiDownloadAttachmentPath({ task_id: taskId, attachment_id: attachmentId })
      const headers = parseApiDownloadAttachmentHeaders({})
      const request = {
        method: "GET" as const,
        path: `/api/v1/tasks/${encodedSegment(path.task_id)}/attachments/${encodedSegment(path.attachment_id)}`,
        headers,
        ...(options.signal === undefined ? {} : { signal: options.signal }),
      }
      return mapDownloadedAttachment(await transport.requestBytes(request))
    },
  }
}
