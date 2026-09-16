import type { ObjectCatalog, ObjectCommand, ObjectPage, ObjectQuery, ObjectReceipt, WorkspaceObject } from '../../domain/objects/model';
export interface ObjectClient {
  resolveBoard(selector:string,signal?:AbortSignal):Promise<string>;
  catalog(signal?:AbortSignal):Promise<ObjectCatalog>;
  list(query:ObjectQuery,signal?:AbortSignal):Promise<ObjectPage>;
  get(boardId:string,id:string,signal?:AbortSignal):Promise<WorkspaceObject>;
  execute(command:ObjectCommand,signal?:AbortSignal):Promise<ObjectReceipt>;
  overview(boardId:string,id:string,signal?:AbortSignal):Promise<unknown>;
  references(query:Record<string,unknown>,signal?:AbortSignal):Promise<unknown>;
  history(boardId:string,id:string,signal?:AbortSignal):Promise<unknown>;
}
export function commandId():string{return `req_${crypto.randomUUID().replaceAll('-','')}`;}
