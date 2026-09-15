import type { WebRuntimeConfig } from "../../lib/runtime";

import { parseApiDownloadAttachmentHeaders } from "../../lib/api/generated/contracts/api-download-attachment-headers";

import { parseApiDownloadAttachmentPath } from "../../lib/api/generated/contracts/api-download-attachment-path";

import { createHttpTransport } from "./http-transport";

import { type AttachmentDownloadDependencies, type AttachmentDownloadClient, encodedSegment, mapDownloadedAttachment } from "../../application/data/attachment-download";

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
