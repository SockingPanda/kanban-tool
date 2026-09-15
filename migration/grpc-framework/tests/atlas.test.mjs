import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const require=createRequire(path.join(root,'web/package.json'));
const ts=require(process.env.TYPESCRIPT_MODULE??'typescript');
const plan=JSON.parse(fs.readFileSync(path.join(root,'integration/patch-plan.json'),'utf8'));
function compile(source){const out=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;const m={exports:{}};new Function('module','exports',out)(m,m.exports);return m.exports}
const {bindBoardRealtime}=compile(fs.readFileSync(path.join(root,'integration/atlas-web/bind.ts'),'utf8'));
function binding(){let context;const events=[];let state='stopped',starts=0,stops=0,retries=0;const source={key:'grpc-test',create(c){context=c;return{start(){starts++;state='connecting'},stop(){stops++;state='stopped'},retry(){retries++;state='connecting'},snapshot(){return{state}}}}};const bound=bindBoardRealtime(source,{boardId:'b_x',boardSelector:'x',record:e=>events.push(e)});return{bound,events,get context(){return context},setState:s=>state=s,counts:()=>({starts,stops,retries})}}
test('branch pin and backend hashes are preserved',()=>{assert.equal(plan.baseBranch,'codex/v4-atlas-paper');assert.equal(plan.baseCommit,'f53e7b884b440f782020587c6f08a7e451757484');assert.equal(plan.files.find(f=>f.path==='crates/kanban-service/src/service.rs').blob,'24736cce35b7d5f2bea3da2fb5ef154ed05bf7d8')});
test('patch never touches visual components, themes, or bootstrap',()=>{for(const f of [...plan.files,...plan.additions]){assert.doesNotMatch(f.path,/src\/(?:features|styles|components|app)\//);assert.doesNotMatch(f.path,/bootstrap|\.css$|vite\.config/)}assert.equal(plan.files.length,10);assert.equal(plan.additions.length,5)});
test('source port has no transport or generated type dependency',()=>{const port=fs.readFileSync(path.join(root,'integration/atlas-web/source.ts'),'utf8');assert.doesNotMatch(port,/^import\s/m);assert.equal(port,fs.readFileSync(path.join(root,'web/src/realtime-port.ts'),'utf8'))});
test('query metadata-only option is preserved',()=>{const hook=plan.files.find(f=>f.path.endsWith('use-board-session.tsx'));for(const edit of hook.edits){assert.doesNotMatch(edit.after,/includeTasks:\s*true/);assert.match(edit.after,/boardRealtime/)} });
test('source binding does not publish data or forge audit events',()=>{const f=binding();f.bound.start();f.context.onRefresh();assert.equal(f.events[0].type,'rpc-refresh-required');assert.equal(f.events[0].cursor,0);assert.equal(f.events[0].details.controlOnly,true);assert.equal('event' in f.events[0].details,false)});
test('late callbacks after release are ignored',()=>{const f=binding();f.bound.start();f.bound.stop();f.context.onRefresh();f.context.onState('live');assert.equal(f.events.length,0)});
test('source binding maps failures to existing UI states',()=>{const f=binding();f.bound.start();f.context.onState('connecting');f.context.onState('retrying');f.context.onState('failed');assert.deepEqual(f.events.map(e=>e.type),['rpc-connecting','transport-failure','circuit-open']);f.setState('failed');assert.equal(f.bound.snapshot().state,'circuit-open');f.setState('retrying');assert.equal(f.bound.snapshot().state,'recovering')});
test('manual retry reuses source controller',()=>{const f=binding();f.bound.start();f.bound.retry();assert.deepEqual(f.counts(),{starts:1,stops:0,retries:1})});
test('refresh telemetry enters query invalidation but not event parsing',()=>{const edit=plan.files.find(f=>f.path.endsWith('session-events.ts')).edits[0];assert.match(edit.after,/"rpc-refresh-required"/);assert.doesNotMatch(edit.after,/parsePollingEnvelope|event_id|event-applied/)});
function choice(){const record={rpc:0,sse:0,reads:0};const edit=plan.files.find(f=>f.path.endsWith('board-session-registry.ts')).edits.find(e=>e.before.includes('const controllerOptions ='));
 const source=`export function create(args) { const {resource,dependencies={},bindBoardRealtime,WebSyncController,createEventsApiClient,createBoardSyncSink}=args;const boardId='b_x',model={board:{slug:'x'}},listeners=new Set(),telemetryListeners=new Set(),sessionTelemetryObservers=new Map(),key='k';${edit.after};return controller;}`;
 const make=compile(source).create;
 const args={resource:{},bindBoardRealtime(){record.rpc++;return{rpc:true}},WebSyncController:class{constructor(){record.sse++}},createEventsApiClient(){record.reads++;return{}},createBoardSyncSink(){return{}}};
 return{make,args,record}}
test('actual registry replacement chooses RPC without constructing legacy helpers',()=>{const f=choice();f.args.resource.boardRealtime={key:'grpc'};assert.equal(f.make(f.args).rpc,true);assert.deepEqual(f.record,{rpc:1,sse:0,reads:0})});
test('actual registry replacement keeps unconfigured legacy path available',()=>{const f=choice();f.make(f.args);assert.deepEqual(f.record,{rpc:0,sse:1,reads:1})});
test('RPC construction error does not fall back to SSE',()=>{const f=choice();f.args.resource.boardRealtime={key:'grpc'};f.args.bindBoardRealtime=()=>{throw new Error('RPC failed')};assert.throws(()=>f.make(f.args),/RPC failed/);assert.equal(f.record.sse,0)});
test('metadata scope mixing guard exists before reuse',()=>{const edits=plan.files.find(f=>f.path.endsWith('board-session-registry.ts')).edits;assert.ok(edits.some(e=>e.after.includes('session.realtimeKey !== realtimeKey')));assert.ok(edits.some(e=>e.after.includes('snapshot?.state === "live"')))});
test('new helper typechecks against the registry state and telemetry shape fixture',()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'atlas-type-'));try {
  fs.mkdirSync(path.join(dir,'realtime'));fs.mkdirSync(path.join(dir,'sync'));
  fs.copyFileSync(path.join(root,'integration/atlas-web/source.ts'),path.join(dir,'realtime/source.ts'));
  fs.copyFileSync(path.join(root,'integration/atlas-web/bind.ts'),path.join(dir,'realtime/bind.ts'));
  fs.writeFileSync(path.join(dir,'sync/contracts.ts'),`export type CanonicalBoardId=string & {readonly __brand:"CanonicalBoardId"};export interface SyncTelemetryEntry{readonly type:string;readonly boardId:CanonicalBoardId;readonly cursor:number;readonly details?:Readonly<Record<string,unknown>>}`);
  fs.writeFileSync(path.join(dir,'check.ts'),`import {bindBoardRealtime} from './realtime/bind';type Expected={start():void;stop():void;retry():void;snapshot?:()=>{state:'idle'|'connecting'|'live'|'recovering'|'polling'|'circuit-open'|'stopped'}};export const assign=(source:ReturnType<typeof bindBoardRealtime>):Expected=>source;`);
  const program=ts.createProgram([path.join(dir,'check.ts')],{strict:true,noEmit:true,exactOptionalPropertyTypes:true,skipLibCheck:true,target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext,moduleResolution:ts.ModuleResolutionKind.Bundler});
  assert.deepEqual(ts.getPreEmitDiagnostics(program).map(d=>ts.flattenDiagnosticMessageText(d.messageText,'\n')),[]);
 }finally{fs.rmSync(dir,{recursive:true,force:true})}
});
