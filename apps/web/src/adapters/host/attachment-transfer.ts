import type { WebRuntimeConfig } from '../../lib/runtime';
import type { RpcTransportOptions } from '../../application/data/rpc-transport';
import type { AttachmentTransferClient, TransferredAttachment } from '../../application/data/attachment-transfer';
import { create } from '@bufbuild/protobuf';
import { FileInfoSchema, FileOwnerInputSchema } from '../../generated/rpc/kanban/extensions/v1/workspace_pb';
import { array, int64, integerValue, record, text } from '../../lib/rpc/value-codec';
import type { QueryRegistry } from './query-registry';
import type { RpcTransport } from '../../application/data/rpc-transport';
import type { FileInfo } from '../../generated/rpc/kanban/extensions/v1/workspace_pb';
import { createRpcClients } from '../../lib/rpc/client';
import { sameOriginRpcBase } from '../../lib/rpc/endpoint';
import { createObjectClient } from './object-client';
import { hashBlob, Sha256 } from '../../domain/files/sha256';
import { MAX_FILE_BYTES } from '../../application/tasks/attachment-queue';

function number(value:bigint):number {const n=Number(value);if(!Number.isSafeInteger(n))throw new Error('file.invalid_integer');return n;}
export function transferred(info:FileInfo):TransferredAttachment {
  if(info.sizeBytes>BigInt(MAX_FILE_BYTES)||!/^a_/.test(info.id))throw new Error('file.invalid_receipt');
  return{id:info.id,board_id:info.boardId,task_id:info.ownerId,filename:info.filename,rel_path:'',content_type:info.contentType||null,size_bytes:number(info.sizeBytes),sha256:info.sha256||null,created_by:info.createdBy,created_at:integerValue(info.createdAt)};
}
export function createAttachmentTransferClient(runtime:WebRuntimeConfig,boardSelector:string,registry:QueryRegistry,transport:RpcTransport,options:RpcTransportOptions={}):AttachmentTransferClient {
  const rpc=createRpcClients(sameOriginRpcBase(runtime,options.documentBaseURI).href,options.fetcher).files;
  const objects=createObjectClient(runtime,registry,transport,options);
  // 失败或取消的看板解析不污染后续上传。
  let resolved:string|undefined;
  async function board(signal?:AbortSignal){return resolved??(resolved=await objects.resolveBoard(boardSelector,signal));}
  return{
    async upload(ownerId,file,fileId,{signal,onProgress}){
      if(file.size>MAX_FILE_BYTES)throw new Error('file.too_large');
      const sha256=await hashBlob(file,signal),boardId=await board(signal);
      const begin=await rpc.beginFileUpload({boardId,ownerId,fileId,filename:file.name,contentType:file.type,sizeBytes:BigInt(file.size),sha256,actor:runtime.actor},{signal,timeoutMs:30000});
      if(begin.completed){
        const value=begin.completed;
        if(value.id!==fileId||value.ownerId!==ownerId||value.boardId!==boardId||value.sha256!==sha256||value.sizeBytes!==BigInt(file.size))throw new Error('file.invalid_receipt');
        return transferred(value);
      }
      if(!begin.uploadId||begin.chunkLimit<1||begin.chunkLimit>65536)throw new Error('file.invalid_session');
      const uploadId=begin.uploadId;
      const cancel=()=>{void rpc.cancelFileUpload({uploadId},{timeoutMs:5000}).catch(()=>{});};
      signal.addEventListener('abort',cancel,{once:true});
      try{
        signal.throwIfAborted();let offset=number(begin.offset);
        if(offset<0||offset>file.size)throw new Error('file.invalid_offset');
        onProgress(offset,file.size);
        while(offset<file.size){
          signal.throwIfAborted();const data=new Uint8Array(await file.slice(offset,offset+begin.chunkLimit).arrayBuffer());
          const next=await rpc.writeFileChunk({uploadId,offset:BigInt(offset),data},{signal,timeoutMs:30000});
          if(next.offset!==BigInt(offset+data.length))throw new Error('file.invalid_offset');
          offset=number(next.offset);onProgress(offset,file.size);
        }
        const result=await rpc.finishFileUpload({uploadId},{signal,timeoutMs:120000});
        if(result.id!==fileId||result.ownerId!==ownerId||result.boardId!==boardId||result.sha256!==sha256||result.sizeBytes!==BigInt(file.size))throw new Error('file.invalid_receipt');
        return transferred(result);
      }finally{signal.removeEventListener('abort',cancel);}
    },
    async list(ownerId,signal){
      const boardId=await board(signal);const result:TransferredAttachment[]=[];let afterId='';const cursors=new Set<string>();
      do{
        const value=record((await registry.read({method:'ExtensionQuery',definition:{case:'listObjectFiles',value:create(FileOwnerInputSchema,{boardId,ownerId,limit:200,afterId})},signal})).payload);
        const page={items:array(value.items,item=>{const v=record(item);return create(FileInfoSchema,{id:text(v.id),boardId:text(v.boardId),ownerId:text(v.ownerId),filename:text(v.filename),contentType:text(v.contentType),sizeBytes:int64(v.sizeBytes),sha256:text(v.sha256),createdBy:text(v.createdBy),createdAt:int64(v.createdAt)});}),nextId:text(value.nextId)};
        for(const item of page.items){if(item.boardId!==boardId||item.ownerId!==ownerId)throw new Error('file.invalid_scope');result.push(transferred(item));}
        if(result.length>10000)throw new Error('file.list_limit');
        afterId=page.nextId;if(afterId&&cursors.has(afterId))throw new Error('file.invalid_cursor');cursors.add(afterId);
      }while(afterId);
      return result;
    },
    async download(ownerId,fileId,signal){
      const boardId=await board(signal),parts:ArrayBuffer[]=[],hash=new Sha256();
      let header:FileInfo|undefined,offset=0n,complete=false;
      for await(const item of rpc.downloadFile({boardId,ownerId,fileId},{signal,timeoutMs:120000})){
        const frame=item.frame;
        if(complete)throw new Error('file.trailing_frame');
        if(frame.case==='header'){
          if(header||frame.value.id!==fileId||frame.value.ownerId!==ownerId||frame.value.boardId!==boardId||frame.value.sizeBytes>BigInt(MAX_FILE_BYTES))throw new Error('file.invalid_header');
          header=frame.value;
        }else if(frame.case==='chunk'){
          if(!header||frame.value.offset!==offset||frame.value.data.length<1||frame.value.data.length>65536)throw new Error('file.invalid_chunk');
          offset+=BigInt(frame.value.data.length);if(offset>header.sizeBytes)throw new Error('file.too_large');
          hash.update(frame.value.data);parts.push(Uint8Array.from(frame.value.data).buffer);
        }else if(frame.case==='complete'){
          if(!header||offset!==header.sizeBytes||offset!==frame.value.sizeBytes||hash.hex()!==frame.value.sha256||(header.sha256&&header.sha256!==frame.value.sha256))throw new Error('file.integrity_failed');
          complete=true;
        }else throw new Error('file.invalid_frame');
      }
      if(!header||!complete)throw new Error('file.download_incomplete');
      // 以下载交付文件，不执行上传的 HTML 或 SVG。
      return{blob:new Blob(parts,{type:'application/octet-stream'}),filename:header.filename};
    },
  };
}
