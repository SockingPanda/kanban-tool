import { loadExplorerBoardIdentity } from './explorer-read-model';
import type { WebRuntimeConfig } from '../../lib/runtime';
import type { RpcTransportOptions } from '../../application/data/rpc-transport';
import type { CatalogRelation, CatalogType, ObjectCatalog, ObjectPage, ObjectReceipt, ObjectValue, ObjectVersion, WorkspaceObject } from '../../domain/objects/model';
import { createRpcClients } from '../../lib/rpc/client';
import { sameOriginRpcBase } from '../../lib/rpc/endpoint';
import { create } from '@bufbuild/protobuf';
import { EmptySchema } from '../../generated/rpc/kanban/v1/common_pb';
import * as messages from '../../generated/rpc/kanban/extensions/v1/workspace_pb';
import type { ObjectClient } from '../../application/data/object-client';
import type { RpcTransport } from '../../application/data/rpc-transport';
import type { QueryRegistry } from './query-registry';
import type { ExtensionQueryCall } from '../../lib/rpc/query-codec';
import { array, bool, decodeJson, encodeJson, int64, integerValue, record, required, text } from '../../lib/rpc/value-codec';

function version(input:unknown):ObjectVersion {const v=record(input);return{object:integerValue(int64(v.object)),source:v.source===null?null:integerValue(int64(v.source))};}
function scalar(input:unknown):ObjectValue {
  const v=record(input),kind=text(v.kind),value=v.value;
  if(!['text','number','boolean','date','object','select'].includes(kind))throw new Error('object.invalid_property_kind');
  if(kind==='date')return{kind,value:integerValue(int64(value))};
  if(kind==='number'){if(typeof value!=='number'||!Number.isFinite(value))throw new Error('object.invalid_number');return{kind,value};}
  if(kind==='boolean')return{kind,value:bool(value)};
  return{kind:kind as 'text'|'object'|'select',value:text(value)};
}
function object(input:unknown):WorkspaceObject {
  const v=record(input);
  return{id:text(v.id),board_id:text(v.board_id),type_key:text(v.type_key),title:text(v.title),body:v.body===null?null:text(v.body),version:version(v.version),created_at:integerValue(int64(v.created_at)),updated_at:integerValue(int64(v.updated_at)),archived_at:v.archived_at===null?null:integerValue(int64(v.archived_at)),properties:Object.fromEntries(Object.entries(record(v.properties)).map(([k,a])=>[k,array(a,scalar)]))};
}
function type(input:unknown):CatalogType {
  const v=record(input),d=record(v.definition);return{definition:{key:text(d.key),name:text(d.name),layout:d.layout},version:integerValue(int64(v.version)),retired:bool(v.retired),system:bool(v.system),capability:text(v.capability)};
}
function relation(input:unknown):CatalogRelation {
  const v=record(input);const cardinality=(v:unknown)=>{if(v!=='one'&&v!=='many')throw new Error('object.invalid_cardinality');return v;};
  return{key:text(v.key),name:text(v.name),source_type:text(v.source_type),target_type:v.target_type===null?null:text(v.target_type),source_property:text(v.source_property),target_property:v.target_property===null?null:text(v.target_property),source_cardinality:cardinality(v.source_cardinality),target_cardinality:cardinality(v.target_cardinality),acyclic:bool(v.acyclic)};
}
export function createObjectClient(runtime:WebRuntimeConfig,registry:QueryRegistry,transport:RpcTransport,options:RpcTransportOptions={}):ObjectClient {
  const rpc=createRpcClients(sameOriginRpcBase(runtime,options.documentBaseURI).href,options.fetcher).objects;
  const read=async(definition:ExtensionQueryCall['definition'],signal?:AbortSignal)=>(await registry.read({method:'ExtensionQuery',definition,signal})).payload;
  return{
    async resolveBoard(selector,signal){return (await loadExplorerBoardIdentity(runtime,selector,{transport,signal,includeArchived:true})).id;},
    async catalog(signal):Promise<ObjectCatalog>{const v=record(await read({case:'getObjectCatalog',value:create(EmptySchema)},signal));return{version:integerValue(int64(v.version)),types:array(v.types,type),relations:array(v.relations,relation),properties:array(v.properties,x=>x),bindings:array(v.bindings,x=>x),workflows:array(v.workflows,x=>x),rollups:array(v.rollups,x=>x)};},
    async list(q,signal):Promise<ObjectPage>{const v=record(await read({case:'listObjects',value:create(messages.ObjectListInputSchema,{boardId:q.board_id,typeKey:q.type_key??'',text:q.text??'',includeArchived:q.include_archived??false,limit:q.limit??100,filters:encodeJson(q.filters??[]),...(q.after?{after:encodeJson(q.after)}:{})})},signal));const next=v.next===null?null:record(v.next);return{items:array(v.items,object),next:next?{query_hash:text(next.query_hash),after_id:text(next.after_id)}:null};},
    async get(boardId,objectId,signal){return object(await read({case:'getObject',value:create(messages.ObjectIdentityInputSchema,{boardId,objectId})},signal));},
    async execute(c,signal):Promise<ObjectReceipt>{const response=await rpc.executeObject({boardId:c.board_id,actor:c.actor,requestId:c.request_id,mutation:encodeJson(c.mutation)},{signal,timeoutMs:30000});const v=record(decodeJson(required(response.data)));return{request_id:text(v.request_id),ids:array(v.ids,text),event_sequence:integerValue(int64(v.event_sequence)),catalog_version:integerValue(int64(v.catalog_version)),replayed:bool(v.replayed),versions:Object.fromEntries(Object.entries(record(v.versions)).map(([id,v])=>[id,version(v)]))};},
    overview: (boardId,objectId,signal)=>read({case:'getObjectOverview',value:create(messages.ObjectIdentityInputSchema,{boardId,objectId})},signal),
    references: (query,signal)=>read({case:'getObjectReferences',value:create(messages.ObjectReferencesInputSchema,{query:encodeJson(query)})},signal),
    history: (boardId,objectId,signal)=>read({case:'getObjectHistory',value:create(messages.ObjectHistoryInputSchema,{boardId,objectId,after:0n,limit:100})},signal),
  };
}
