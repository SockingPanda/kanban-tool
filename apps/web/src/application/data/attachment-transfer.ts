import type { ApiCreateAttachmentResponseContract } from '../../lib/api/generated/contracts/api-create-attachment-response';
export type TransferredAttachment = ApiCreateAttachmentResponseContract['data'];
export interface AttachmentUploadOptions { readonly signal:AbortSignal;readonly onProgress:(sentBytes:number,totalBytes:number)=>void }
/** `taskId` 保留旧队列参数名；所属对象可以是同一看板内的任意对象。 */
export interface AttachmentTransferClient {
  upload(taskId:string,file:File,attachmentId:string,options:AttachmentUploadOptions):Promise<TransferredAttachment>;
  list(ownerId:string,signal?:AbortSignal):Promise<readonly TransferredAttachment[]>;
  download(ownerId:string,attachmentId:string,signal?:AbortSignal):Promise<{blob:Blob;filename:string}>;
}
