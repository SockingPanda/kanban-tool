import { useCallback, useEffect, useMemo, useRef, useState, useReducer } from 'react';
import { Code, ConnectError } from '@connectrpc/connect';
import { parseJson } from '../../lib/lossless-json';
import { Button } from '../../components/ui/button';
import { Dialog } from '../../components/ui/dialog';
import { useWorkspaceOperations } from '../../application/workspace/use-workspace-operations';
import { useI18n } from '../../platform/localization/use-i18n';
import { usePreferences } from '../../platform/preferences/use-preferences';
import type { Translator } from '../../platform/localization/contracts.generated';
import type { WebRuntimeConfig } from '../../lib/runtime';
import { target, type CatalogType, type ObjectCatalog, type ObjectCommand, type ObjectPage, type ObjectReceipt, type RelationLink, type WorkspaceObject } from '../../domain/objects/model';
import { commandId, type ObjectClient } from '../../application/data/object-client';
import { observeRead, refreshRead } from '../../application/query/observe-read';
import { AttachmentManager } from '../attachments';
import type { AttachmentTransferClient, TransferredAttachment } from '../../application/data/attachment-transfer';
import styles from './objects.module.css';

function ignore(work:Promise<unknown>){void work.catch(()=>{});}
function errorText(error:unknown){return error instanceof Error?error.message:String(error);}
function json(value:unknown){return JSON.stringify(value,(_,v:unknown)=>typeof v==='bigint'?v.toString():v,2);}
function name(type:CatalogType,t:Translator){
 const stock:Record<string,string>={task:'任务',module:'模块',cycle:'迭代',file:'文件',note:'笔记',resource:'资料'};
 if(type.definition.name!==stock[type.definition.key])return type.definition.name;
 switch(type.definition.key){case'task':return t('objects:task');case'module':return t('objects:module');case'cycle':return t('objects:cycle');case'file':return t('objects:file');case'note':return t('objects:note');case'resource':return t('objects:resource');default:return type.definition.name;}
}
function refs(object:WorkspaceObject,key:string){return(object.properties[key]??[]).flatMap(v=>v.kind==='object'?[String(v.value)]:[]);}
function readOnly(object:WorkspaceObject){return object.archived_at!==null||['completed','cancelled'].includes(String(object.properties['cycle.status']?.[0]?.value));}
function definitive(error:unknown){return error instanceof ConnectError&&[Code.InvalidArgument,Code.NotFound,Code.Aborted,Code.FailedPrecondition,Code.PermissionDenied,Code.Unauthenticated].includes(error.code);}

type Patch<S> = Partial<S> | ((state:S)=>Partial<S>);
function mergeState<S>(state:S,patch:Patch<S>):S {return {...state,...(typeof patch==='function'?patch(state):patch)};}
interface WorkspaceState {
 board?:string;catalog?:ObjectCatalog;page:ObjectPage;typeKey:string;search:string;query:string;archived:boolean;
 selected?:string;object?:WorkspaceObject;loading:boolean;error?:string;tick:number;connection:'connecting'|'live'|'stale';
 createOpen:boolean;typeOpen:boolean;busy:boolean;uploadBusy:boolean;pending?:ObjectCommand;
}
interface DetailState {
 title:string;body:string;titleDirty:boolean;bodyDirty:boolean;titleBase:WorkspaceObject;bodyBase:WorkspaceObject;
 error:string;overview?:unknown;audit?:unknown;auditOpen:boolean;carry:string;action:'start_workflow'|'close_workflow'|'cancel_workflow'|'archive'|null;
}

type Run=(mutation:Record<string,unknown>)=>Promise<ObjectReceipt>;
export interface ObjectWorkspaceProps { runtime:WebRuntimeConfig;boardSelector:string;initialType?:string }
function useObjectWorkspace({runtime,boardSelector,initialType='module'}:ObjectWorkspaceProps){
 const {t,locale}=useI18n(),preferences=usePreferences(),source=useWorkspaceOperations();
 const configured=useMemo(()=>({...runtime,actor:preferences.actor||runtime.actor}),[runtime,preferences.actor]);
 const client=useMemo(()=>source.createObjectClient(configured),[source,configured]);
 const files=useMemo(()=>source.createAttachmentTransferClient(configured,boardSelector),[source,configured,boardSelector]);
 const [state,setView]=useReducer(mergeState<WorkspaceState>,{page:{items:[],next:null},typeKey:initialType,search:'',query:'',archived:false,loading:false,tick:0,connection:'connecting',createOpen:false,typeOpen:false,busy:false,uploadBusy:false});
 const {board,catalog,page,typeKey,search,query,archived,selected,object,loading,error,tick,connection,createOpen,typeOpen,busy,uploadBusy,pending}=state;
 const pendingRef=useRef<ObjectCommand|undefined>(undefined);
 const pendingClient=useRef<ObjectClient|undefined>(undefined),pageEpoch=useRef(0),pageAbort=useRef<AbortController|undefined>(undefined);
 const onUploadBusy=useCallback((uploadBusy:boolean)=>setView({uploadBusy}),[]);
 const refresh=useCallback(()=>setView(state=>({tick:state.tick+1})),[]);
 useEffect(()=>{const timer=setTimeout(()=>setView({query:search}),200);return()=>clearTimeout(timer);},[search]);
 useEffect(()=>{
 if(!client)return;const abort=new AbortController();setView({board:undefined});setView({object:undefined});setView({page:{items:[],next:null}});
  void client.resolveBoard(boardSelector,abort.signal).then(value=>{if(!abort.signal.aborted)setView({board:value});}).catch(e=>{if(!abort.signal.aborted)setView({error:errorText(e)});});return()=>abort.abort();
 },[client,boardSelector]);
 useEffect(()=>{
  if(!client||!board)return;const abort=new AbortController();pageEpoch.current++;pageAbort.current?.abort();setView({loading:true});setView({connection:'connecting'});
  observeRead(signal=>Promise.all([client.catalog(signal),client.list({board_id:board,type_key:typeKey,text:query,include_archived:archived,limit:100},signal),selected?client.get(board,selected,signal):Promise.resolve(undefined)]),abort.signal,
   ([catalog,page,object])=>{setView({catalog:catalog});setView({page:page});setView({object:object});setView({error:undefined});setView({loading:false});setView({connection:'live'});},
   e=>{setView({error:errorText(e)});setView({loading:false});setView({connection:'stale'});},tick>0);
  return()=>{abort.abort();pageAbort.current?.abort();};
 },[client,board,typeKey,query,archived,selected,tick]);
 const submit=useCallback(async(command:ObjectCommand)=>{
  const sender=pendingClient.current??client;if(!sender)throw new Error('object.unavailable');pendingClient.current=sender;setView({busy:true});setView({error:undefined});pendingRef.current=command;setView({pending:command});
  try{
   const receipt=await sender.execute(command);pendingClient.current=undefined;pendingRef.current=undefined;setView({pending:undefined});
   // 写入已确认；在释放操作按钮前以 Ready 核对新版本，避免连续动作使用旧 CAS。
   try {
    const [catalog,object]=await refreshRead(signal=>Promise.all([sender.catalog(signal),selected?sender.get(command.board_id,selected,signal):Promise.resolve(undefined)]),AbortSignal.timeout(30000));
    setView({catalog,object});
   } catch(error) { setView({error:errorText(error)}); }
   refresh();return receipt;
  }
  catch(e){if(definitive(e)){pendingClient.current=undefined;pendingRef.current=undefined;setView({pending:undefined});}setView({error:errorText(e)});throw e;}
  finally{setView({busy:false});}
 },[client,refresh,selected]);
 const run:Run=useCallback(async(mutation)=>{
  if(!board||busy||pendingRef.current)throw new Error('object.command_pending');
  return submit({board_id:board,actor:configured.actor,request_id:commandId(),mutation});
 },[board,busy,configured.actor,submit]);
 const more=async()=>{
  if(!client||!board||!page.next||loading)return;
  const epoch=pageEpoch.current,abort=new AbortController();pageAbort.current=abort;setView({loading:true});
  try{const next=await client.list({board_id:board,type_key:typeKey,text:query,include_archived:archived,limit:100,after:page.next},abort.signal);
   if(epoch!==pageEpoch.current||abort.signal.aborted)return;
   setView(state=>({page:((p:ObjectPage)=>({items:[...p.items,...next.items.filter(n=>!p.items.some(o=>o.id===n.id))],next:next.next}))(state.page)}));
  }catch(e){if(epoch===pageEpoch.current&&!abort.signal.aborted)setView({error:errorText(e)});}
  finally{if(epoch===pageEpoch.current&&!abort.signal.aborted)setView({loading:false});}
 };
 useEffect(()=>{
  if(!pending&&!uploadBusy)return;
  const warn=(event:BeforeUnloadEvent)=>{event.preventDefault();event.returnValue='';};
  window.addEventListener('beforeunload',warn);return()=>window.removeEventListener('beforeunload',warn);
 },[pending,uploadBusy]);
 const disabled=busy||Boolean(pending)||uploadBusy;
  return {t,locale,client,files,board,catalog,page,typeKey,search,archived,selected,object,loading,error,connection,createOpen,typeOpen,busy,pending,refresh,submit,run,more,setView,disabled,onUploadBusy};
}
export function ObjectWorkspace(props:ObjectWorkspaceProps){
 const model=useObjectWorkspace(props);
 const {t,locale,client,files,board,catalog,typeKey,object,error,createOpen,typeOpen,busy,pending,refresh,submit,run,setView,disabled,onUploadBusy}=model;
 if(!client)return <section role="status">{t('objects:unavailable')}</section>;
 return <section className={styles.workspace} data-testid="object-workspace">
  <ObjectHeader model={model}/>
  {error&&<div role="alert" className={styles.error}><p>{t(pending?'objects:unknownCommit':'objects:writeError')}</p><details><summary>{t('objects:audit')}</summary><pre>{error}</pre></details></div>}
  {pending&&<div role="status" className={styles.notice}><span>{t('objects:pendingId')} </span><code translate="no">{pending.request_id}</code><Button disabled={busy} onClick={()=>ignore(submit(pending))}>{t('objects:retryOriginal')}</Button></div>}
  <div className={styles.columns}><ObjectList model={model}/><main className={styles.detail}>
   {object&&catalog&&board?<ObjectDetail key={object.id} object={object} catalog={catalog} client={client} files={files} run={run} disabled={disabled} refresh={refresh} locale={locale} onUploadBusy={onUploadBusy}/>:<p>{t('objects:choose')}</p>}
  </main></div>
  {catalog&&<CreateObject open={createOpen} typeKey={typeKey} catalog={catalog} disabled={disabled} run={run} onClose={()=>setView({createOpen:false})} onCreated={id=>{setView({selected:id});setView({createOpen:false});}}/>}
  {catalog&&<CreateType open={typeOpen} catalog={catalog} disabled={disabled} run={run} onClose={()=>setView({typeOpen:false})}/>}
 </section>;
}

function ObjectHeader({model}:{model:ReturnType<typeof useObjectWorkspace>}){
 const {t,connection,loading,refresh,disabled,catalog,typeKey,setView}=model;
 return <>  <header className={styles.header}><div><h1>{t('objects:title')}</h1><p role="status">{t(connection==='live'?'objects:live':connection==='stale'?'objects:stale':'objects:connecting')}</p></div><div className={styles.actions}><Button variant="secondary" disabled={loading} onClick={refresh}>{t('objects:refresh')}</Button><Button disabled={disabled||!catalog||['file','task'].includes(typeKey)} onClick={()=>setView({createOpen:true})}>{t('objects:create')}</Button><Button variant="secondary" disabled={disabled||!catalog} onClick={()=>setView({typeOpen:true})}>{t('objects:createType')}</Button></div></header>
  <nav className={styles.types} aria-label={t('objects:selectType')}>{catalog?.types.flatMap(type=>type.retired||type.definition.key==='task'?[]:[<Button key={type.definition.key} variant="secondary" aria-pressed={typeKey===type.definition.key} disabled={disabled} onClick={()=>{setView({typeKey:type.definition.key});setView({selected:undefined});setView({object:undefined});}}>{name(type,t)}</Button>])}</nav></>;
}
function ObjectList({model}:{model:ReturnType<typeof useObjectWorkspace>}){
 const {t,search,disabled,setView,archived,loading,page,selected,more}=model;
 return <aside className={styles.list}>
   <label className={styles.field}>{t('objects:search')}<input value={search} disabled={disabled} onChange={e=>setView({search:e.target.value})}/></label>
   <label className={styles.checkbox}><input type="checkbox" checked={archived} disabled={disabled} onChange={e=>setView({archived:e.target.checked})}/>{t('objects:showArchived')}</label>
   {loading&&<p role="status">{t('objects:loading')}</p>}
   {!loading&&page.items.length===0&&<p>{t('objects:empty')}</p>}
   {page.items.map(item=><button type="button" className={styles.item} key={item.id} aria-pressed={selected===item.id} disabled={disabled} onClick={()=>setView({selected:item.id})}><strong>{item.title}</strong><code translate="no">{item.id}</code>{item.archived_at!==null&&<span>{t('objects:archived')}</span>}</button>)}
   {page.next&&<Button disabled={loading||disabled} onClick={()=>ignore(more())}>{t('objects:more')}</Button>}
  </aside>;
}

function CreateObject({open,typeKey,catalog,disabled,run,onClose,onCreated}:{open:boolean;typeKey:string;catalog:ObjectCatalog;disabled:boolean;run:Run;onClose:()=>void;onCreated:(id:string)=>void}){
 const {t}=useI18n();const [title,setTitle]=useState(''),[body,setBody]=useState(''),[starts,setStarts]=useState(''),[ends,setEnds]=useState(''),[properties,setProperties]=useState('{}'),[error,setError]=useState('');
 const submit=async()=>{try{const values:unknown=parseJson(properties);if(values===null||typeof values!=='object'||Array.isArray(values))throw new Error(t('objects:badProperties'));const props={...values};if(typeKey==='cycle'){const start=Date.parse(starts),end=Date.parse(ends);if(!Number.isFinite(start)||!Number.isFinite(end)||end<=start)throw new Error(t('objects:badDate'));Object.assign(props,{'cycle.starts_at':[{kind:'date',value:start}],'cycle.ends_at':[{kind:'date',value:end}]});}const receipt=await run({operation:'create',object:{type_key:typeKey,title,body:body||null,properties:props,expected_catalog_version:catalog.version}});onCreated(receipt.ids[0]);setTitle('');setBody('');setProperties('{}');setError('');}catch(e){setError(errorText(e));}};
 return <Dialog open={open} onClose={()=>{if(!disabled)onClose();}} title={t('objects:create')}><form className={styles.form} onSubmit={e=>{e.preventDefault();void submit();}}><label className={styles.field}>{t('objects:name')}<input value={title} required maxLength={200} disabled={disabled} onChange={e=>setTitle(e.target.value)}/></label><label className={styles.field}>{t('objects:body')}<textarea value={body} disabled={disabled} onChange={e=>setBody(e.target.value)}/></label>{typeKey==='cycle'&&<><label className={styles.field}>{t('objects:starts')}<input type="datetime-local" required value={starts} disabled={disabled} onChange={e=>setStarts(e.target.value)}/></label><label className={styles.field}>{t('objects:ends')}<input type="datetime-local" required value={ends} disabled={disabled} onChange={e=>setEnds(e.target.value)}/></label></>}<details><summary>{t('objects:customProperties')}</summary><textarea aria-label={t('objects:customProperties')} value={properties} disabled={disabled} onChange={e=>setProperties(e.target.value)}/></details>{error&&<p role="alert">{error}</p>}<Button type="submit" disabled={disabled}>{t('objects:save')}</Button></form></Dialog>;
}
function CreateType({open,catalog,disabled,run,onClose}:{open:boolean;catalog:ObjectCatalog;disabled:boolean;run:Run;onClose:()=>void}){
 const {t}=useI18n();const [key,setKey]=useState('custom.'),[name,setName]=useState(''),[error,setError]=useState('');
 return <Dialog open={open} onClose={()=>{if(!disabled)onClose();}} title={t('objects:createType')}><form className={styles.form} onSubmit={e=>{e.preventDefault();void run({operation:'define_type',definition:{key,name,layout:{view:'document',body_format:'markdown'}},expected_catalog_version:catalog.version}).then(onClose).catch(e=>setError(errorText(e)));}}><label className={styles.field}>{t('objects:typeKey')}<input required value={key} disabled={disabled} onChange={e=>setKey(e.target.value)}/></label><label className={styles.field}>{t('objects:typeName')}<input required value={name} disabled={disabled} onChange={e=>setName(e.target.value)}/></label>{error&&<p role="alert">{error}</p>}<Button type="submit" disabled={disabled}>{t('objects:save')}</Button></form></Dialog>;
}
type ObjectDetailProps={object:WorkspaceObject;catalog:ObjectCatalog;client:ObjectClient;files:AttachmentTransferClient|null;run:Run;disabled:boolean;refresh:()=>void;locale:'zh'|'en';onUploadBusy:(busy:boolean)=>void};
function useObjectDetail({object,catalog,client,run,disabled}:Pick<ObjectDetailProps,'object'|'catalog'|'client'|'run'|'disabled'>){
 const {t}=useI18n();
 const [state,setDrafts]=useReducer(mergeState<DetailState>,{title:object.title,body:object.body??'',titleDirty:false,bodyDirty:false,titleBase:object,bodyBase:object,error:'',auditOpen:false,carry:'',action:null});
 const {title,body,titleDirty,bodyDirty,titleBase,bodyBase,error,overview,audit,auditOpen,carry,action}=state;
 useEffect(()=>{
  // 外部版本仅刷新未编辑的草稿；已编辑草稿必须保留原 CAS 基线。
  // react-doctor-disable-next-line react-doctor/no-adjust-state-on-prop-change
  if(!titleDirty&&BigInt(object.version.object)>=BigInt(titleBase.version.object)){setDrafts({title:object.title});setDrafts({titleBase:object});}
  // react-doctor-disable-next-line react-doctor/no-adjust-state-on-prop-change
  if(!bodyDirty&&BigInt(object.version.object)>=BigInt(bodyBase.version.object)){setDrafts({body:object.body??''});setDrafts({bodyBase:object});}
 },[object,titleDirty,bodyDirty,titleBase.version.object,bodyBase.version.object]);
 useEffect(()=>{if(!['module','cycle'].includes(object.type_key)){setDrafts({overview:undefined});return;}const abort=new AbortController();observeRead(signal=>client.overview(object.board_id,object.id,signal),abort.signal,overview=>setDrafts({overview}),e=>setDrafts({error:errorText(e)}));return()=>abort.abort();},[client,object.board_id,object.id,object.type_key]);
 useEffect(()=>{if(!auditOpen)return;const abort=new AbortController();observeRead(signal=>client.history(object.board_id,object.id,signal),abort.signal,audit=>setDrafts({audit}),e=>setDrafts({error:errorText(e)}));return()=>abort.abort();},[client,object.board_id,object.id,auditOpen]);
 const save=async(field:'title'|'body')=>{
  const base=field==='title'?titleBase:bodyBase;
  try{
   const receipt=await run(field==='title'?{operation:'patch',patch:{target:target(base),title,edits:[],expected_catalog_version:catalog.version}}:{operation:'set_body',target:target(base),body:body||null});
   const acknowledged=receipt.versions[object.id];
   if(!acknowledged)throw new Error('object.invalid_receipt');
   // 仅用本次确认的写入推进未改动草稿；保留其他写入造成的 CAS 冲突。
   const advance=(previous:WorkspaceObject)=>json(previous.version)===json(base.version)?{...previous,version:acknowledged}:previous;
   setDrafts(state=>({titleBase:advance(state.titleBase),bodyBase:advance(state.bodyBase)}));
   if(field==='title')setDrafts({titleDirty:false});else setDrafts({bodyDirty:false});setDrafts({error:''});
  }catch(e){setDrafts({error:errorText(e)});}
 };
 const act=async()=>{if(!action)return;try{if(action==='archive')await run({operation:'set_archived',target:target(object),archived:object.archived_at===null});else if(action==='close_workflow'){const next=carry.trim()?target(await client.get(object.board_id,carry.trim())):null;await run({operation:action,target:target(object),carry_to:next});}else await run({operation:action,target:target(object)});setDrafts({action:null});setDrafts({error:''});}catch(e){setDrafts({error:errorText(e)});}};
 const frozen=readOnly(object),blocked=disabled||frozen;
 return {t,title,body,titleDirty,bodyDirty,titleBase,bodyBase,error,overview,audit,carry,action,setDrafts,save,act,frozen,blocked};
}
function ObjectDetail({object,catalog,client,files,run,disabled,refresh,locale,onUploadBusy}:ObjectDetailProps){
 const model=useObjectDetail({object,catalog,client,run,disabled});
 const {t,titleDirty,bodyDirty,titleBase,bodyBase,error,overview,audit,setDrafts,frozen,blocked}=model;
 return <div className={styles.inspector}><header><code translate="no">{object.id}</code><p>{t('objects:version')} <span translate="no">{String(object.version.object)}</span></p></header>
  {error&&<p role="alert">{error}</p>}
  {((titleDirty&&String(titleBase.version.object)!==String(object.version.object))||(bodyDirty&&String(bodyBase.version.object)!==String(object.version.object)))&&<p role="status">{t('objects:dirtyChanged')}</p>}
  <ObjectDraftEditor object={object} disabled={disabled} model={model}/>
  {object.type_key==='cycle'&&!frozen&&<div className={styles.actions}><Button disabled={disabled} onClick={()=>setDrafts({action:'start_workflow'})}>{t('objects:start')}</Button><Button disabled={disabled} onClick={()=>setDrafts({action:'close_workflow'})}>{t('objects:close')}</Button><Button disabled={disabled} onClick={()=>setDrafts({action:'cancel_workflow'})}>{t('objects:cancelCycle')}</Button></div>}
  {overview!==undefined&&<details open><summary>{t('objects:overview')}</summary><pre>{json(overview)}</pre></details>}
  <details><summary>{t('objects:properties')}</summary><pre>{json(object.properties)}</pre></details>
  <RelationEditor object={object} catalog={catalog} client={client} run={run} disabled={blocked}/>
  {object.type_key==='file'?<section><h3>{t('objects:fileOwners')}</h3>{refs(object,'file.owners').length===0&&<p>{t('objects:noOwners')}</p>}{refs(object,'file.owners').map(id=><p key={id}><code translate="no">{id}</code></p>)}</section>:<ObjectAttachments object={object} catalog={catalog} client={client} files={files} run={run} disabled={blocked} refresh={refresh} locale={locale} onBusy={onUploadBusy}/>}
  <details onToggle={event=>setDrafts({auditOpen:event.currentTarget.open})}><summary>{t('objects:audit')}</summary><pre>{audit===undefined?t('objects:loading'):json(audit)}</pre></details>
  <ObjectActionDialog object={object} disabled={disabled} model={model}/>
 </div>;
}
function ObjectActionDialog({object,disabled,model}:{object:WorkspaceObject;disabled:boolean;model:ReturnType<typeof useObjectDetail>}){
 const {t,error,carry,action,setDrafts,act}=model;
 return (<Dialog open={action!==null} title={t('objects:confirm')} onClose={()=>{if(!disabled)setDrafts({action:null});}}>{error&&<p role="alert">{error}</p>}<p>{action==='archive'?t(object.archived_at===null?'objects:archive':'objects:restore'):t('objects:closureHint')}</p>{action==='close_workflow'&&<label className={styles.field}>{t('objects:carryTo')}<input value={carry} disabled={disabled} onChange={e=>setDrafts({carry:e.target.value})}/></label>}<Button disabled={disabled} onClick={()=>{void act();}}>{t('objects:confirm')}</Button></Dialog>);
}
function ObjectDraftEditor({object,disabled,model}:{object:WorkspaceObject;disabled:boolean;model:ReturnType<typeof useObjectDetail>}){
 const {t,title,body,titleDirty,bodyDirty,setDrafts,save,blocked}=model;
 return <>  <label className={styles.field}>{t('objects:name')}<input value={title} maxLength={200} disabled={blocked||object.type_key==='task'} onChange={e=>{setDrafts({title:e.target.value});setDrafts({titleDirty:true});}}/></label><Button disabled={blocked||object.type_key==='task'||title===object.title} onClick={()=>{void save('title');}}>{t('objects:saveTitle')}</Button>
  <label className={styles.field}>{t('objects:body')}<textarea value={body} disabled={blocked||object.type_key==='task'} onChange={e=>{setDrafts({body:e.target.value});setDrafts({bodyDirty:true});}}/></label><div className={styles.actions}><Button disabled={blocked||object.type_key==='task'||body===(object.body??'')} onClick={()=>{void save('body');}}>{t('objects:saveBody')}</Button><Button variant="secondary" disabled={disabled||(!titleDirty&&!bodyDirty)} onClick={()=>{setDrafts({titleDirty:false});setDrafts({bodyDirty:false});setDrafts({title:object.title});setDrafts({body:object.body??''});setDrafts({titleBase:object});setDrafts({bodyBase:object});}}>{t('objects:discardDraft')}</Button><Button variant="secondary" disabled={disabled||object.type_key==='task'} onClick={()=>setDrafts({action:'archive'})}>{t(object.archived_at===null?'objects:archive':'objects:restore')}</Button></div>
</>;
}

function RelationEditor({object,catalog,client,run,disabled}:{object:WorkspaceObject;catalog:ObjectCatalog;client:ObjectClient;run:Run;disabled:boolean}){
 const {t}=useI18n();const definitions=catalog.relations.flatMap(d=>[
  ...(d.source_type===object.type_key?[{definition:d,forward:true,key:d.key+':source',property:d.source_property}]:[]),
  ...((d.target_type===object.type_key||d.target_type===null)?[{definition:d,forward:false,key:d.key+':target',property:d.target_property??d.key}]:[]),
 ]);
 const [key,setKey]=useState(''),[id,setId]=useState(''),[replace,setReplace]=useState(false),[error,setError]=useState('');
 const direction=definitions.find(d=>d.key===key)??definitions[0],definition=direction?.definition;
 const change=async(adding:boolean)=>{if(!definition)return;try{
  const other=await client.get(object.board_id,id.trim());const forward=direction!.forward;
  const source=forward?object:other,targetObject=forward?other:object;
  const link:RelationLink={relation_key:definition.key,source_id:source.id,target_id:targetObject.id};
  const remove:RelationLink[]=adding?[]:[link],add=adding?[link]:[];const expected=new Map([[source.id,target(source)],[targetObject.id,target(targetObject)]]);
  if(adding&&replace){
   // 每次仅占用一个 registry 查询，避免大关系集合耗尽 64 个活跃查询预算。
   // react-doctor-disable-next-line react-doctor/async-await-in-loop
   if(definition.source_cardinality==='one')for(const old of refs(source,definition.source_property).filter(x=>x!==targetObject.id)){remove.push({relation_key:definition.key,source_id:source.id,target_id:old});expected.set(old,target(await client.get(object.board_id,old)));}
   // react-doctor-disable-next-line react-doctor/async-await-in-loop
   if(definition.target_cardinality==='one'&&definition.target_property)for(const old of refs(targetObject,definition.target_property).filter(x=>x!==source.id)){remove.push({relation_key:definition.key,source_id:old,target_id:targetObject.id});expected.set(old,target(await client.get(object.board_id,old)));}
  }
  await run({operation:'change_relations',change:{expected_catalog_version:catalog.version,expected:[...expected.values()],remove,add}});setId('');setError('');
 }catch(e){setError(errorText(e));}};
 return <section><h3>{t('objects:relations')}</h3>{definitions.length===0?<p>{t('objects:relatedEmpty')}</p>:<><div className={styles.actions}>{definitions.map(d=><Button key={d.key} variant="secondary" disabled={disabled} aria-pressed={direction?.key===d.key} onClick={()=>setKey(d.key)}><span translate="no">{d.property}</span></Button>)}</div><label className={styles.field}>{t('objects:relatedId')}<input value={id} disabled={disabled} onChange={e=>setId(e.target.value)}/></label><label className={styles.checkbox}><input type="checkbox" disabled={disabled} checked={replace} onChange={e=>setReplace(e.target.checked)}/>{t('objects:replaceSingle')}</label><div className={styles.actions}><Button disabled={disabled||!id.trim()} onClick={()=>{void change(true);}}>{t('objects:addRelation')}</Button><Button variant="secondary" disabled={disabled||!id.trim()} onClick={()=>{void change(false);}}>{t('objects:removeRelation')}</Button></div></>}{error&&<p role="alert">{error}</p>}</section>;
}
function ObjectAttachments({object,catalog,client,files,run,disabled,refresh,locale,onBusy}:{object:WorkspaceObject;catalog:ObjectCatalog;client:ObjectClient;files:AttachmentTransferClient|null;run:Run;disabled:boolean;refresh:()=>void;locale:'zh'|'en';onBusy:(busy:boolean)=>void}){
 const [items,setItems]=useState<readonly TransferredAttachment[]>([]),[error,setError]=useState<string>(),[loading,setLoading]=useState(false);
 const read=useCallback(async()=>{if(!files)return;setLoading(true);try{setItems(await refreshRead(signal=>files.list(object.id,signal)));setError(undefined);}catch(e){setError(errorText(e));throw e;}finally{setLoading(false);}},[files,object.id]);
 useEffect(()=>{if(!files)return;const abort=new AbortController();setLoading(true);observeRead(signal=>files.list(object.id,signal),abort.signal,items=>{setItems(items);setError(undefined);setLoading(false);},e=>{setError(errorText(e));setLoading(false);},true);return()=>abort.abort();},[files,object.id,object.version.object]);
 return <AttachmentManager taskId={object.id} client={files} attachments={items} locale={locale} disabled={disabled} loading={loading} error={error} onBusyChange={onBusy} reconcile={async()=>{await read();refresh();}} remove={async(id)=>{
  const file=await client.get(object.board_id,id);await run({operation:'change_relations',change:{expected_catalog_version:catalog.version,expected:[target(object),target(file)],remove:[{relation_key:'file.attachment',source_id:id,target_id:object.id}],add:[]}});
  try{await read();return{committed:true,reconciled:true};}catch{return{committed:true,reconciled:false};}
 }}/>;
}
