import { type HttpTransport, type HttpTransportBytesResponse, type HttpTransportOptions } from "./http-transport";

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

export function encodedSegment(value: string): string {
  return encodeURIComponent(value)
}

export function mapDownloadedAttachment(response: HttpTransportBytesResponse): DownloadedAttachment {
  return {
    content_type: response.contentType,
    attachment_id: response.attachmentId,
    sha256: response.sha256,
    content: response.bytes,
  }
}
