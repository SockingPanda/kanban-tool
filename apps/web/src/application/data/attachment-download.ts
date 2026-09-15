import type { RpcTransport, RpcTransportOptions } from "./rpc-transport";

export interface AttachmentDownloadOptions {
  readonly signal?: AbortSignal
}

export interface DownloadedAttachment {
  readonly content_type: string | null
  readonly attachment_id: string | null
  readonly sha256: string | null
  readonly content: Uint8Array
}

export interface AttachmentDownloadDependencies extends RpcTransportOptions {
  readonly transport?: RpcTransport
}

export interface AttachmentDownloadClient {
  downloadAttachment(
    taskId: string,
    attachmentId: string,
    options?: AttachmentDownloadOptions,
  ): Promise<DownloadedAttachment>
}
