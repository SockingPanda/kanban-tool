import test from 'node:test';
import assert from 'node:assert/strict';
import { ProjectionStore } from '../dist/reducer.js';
import { WatchSession } from '../dist/watch.js';
const frame=body=>({boardId:'b_x',scope:'board:b_x:cards:v1',epoch:'e1',body});
const begin=revision=>frame({kind:'begin',revision,count:0});
const commit=revision=>frame({kind:'commit',revision,count:0,chunks:0});
const tick=()=>new Promise(r=>setImmediate(r));
const aborted=signal=>signal.aborted?Promise.resolve():new Promise(r=>signal.addEventListener('abort',r,{once:true}));
const deferred=()=>{let resolve;const promise=new Promise(r=>resolve=r);return{promise,resolve}};

test('reconnect uses applied cursor not an unfinished snapshot',async()=>{
  const inputs=[];const store=new ProjectionStore('b_x',()=>{});let session;
  session=new WatchSession(store,async function*(input){inputs.push(input.resume);if(inputs.length===1){yield begin(1n);yield commit(1n);yield begin(2n)}else{session.stop()}},{delay:async()=>{}});
  await session.start();assert.equal(inputs.length,2);assert.equal(inputs[1].revision,1n);
});
test('protocol gap forces snapshot request while preserving visible data',async()=>{
  const store=new ProjectionStore('b_x',()=>{});store.apply(begin(1n));store.apply(commit(1n));const inputs=[];let session;
  session=new WatchSession(store,async function*(input){inputs.push(input.resume);if(inputs.length===1)yield frame({kind:'delta',base:8n,revision:9n,upserts:[],removed:[]});else session.stop()},{delay:async()=>{}});
  await session.start();assert.equal(inputs[1],undefined);assert.equal(store.snapshot().cursor.revision,1n);
});
test('terminal transport error is never retried',async()=>{
  let attempts=0;const store=new ProjectionStore('b_x',()=>{});const session=new WatchSession(store,async function*(){attempts++;throw Error('forbidden')},{terminal:()=>true});
  await assert.rejects(session.start(),/forbidden/);assert.equal(attempts,1);
});
test('EOF retry backoff is capped and eventually stops',async()=>{
  let attempts=0;const delays=[];const session=new WatchSession(new ProjectionStore('b_x',()=>{}),async function*(){attempts++},{delay:async ms=>delays.push(ms),random:()=>0.5});
  await assert.rejects(session.start(),/连续失败/);assert.equal(attempts,8);assert.equal(Math.max(...delays),5000);
});
test('stop aborts the active stream without a new connection',async()=>{
  let signal;let attempts=0;const session=new WatchSession(new ProjectionStore('b_x',()=>{}),async function*(input){attempts++;signal=input.signal;await aborted(signal)});
  const run=session.start();await tick();session.stop();await run;assert.equal(signal.aborted,true);assert.equal(attempts,1);
});
test('old generation cannot clear staging of a new connection',async()=>{
  const old=deferred(), newer=deferred();let calls=0;const store=new ProjectionStore('b_x',()=>{});
  const session=new WatchSession(store,async function*(input){calls++;const n=calls;if(n===1){await old.promise;yield begin(1n)}else{yield begin(2n);await newer.promise;yield commit(2n);await aborted(input.signal)}});
  const first=session.start();await tick();const second=session.start();await tick();old.resolve();await first;newer.resolve();await tick();assert.equal(store.resume().revision,2n);session.stop();await second;
});
test('watchdog cancels a silent stream and retries',async()=>{
  let calls=0;let session;session=new WatchSession(new ProjectionStore('b_x',()=>{}),async function*(input){calls++;if(calls===1)await aborted(input.signal);else session.stop()},{livenessMs:5,delay:async()=>{}});
  await session.start();assert.equal(calls,2);
});
test('stopping during retry delay does not open another RPC',async()=>{
  let calls=0;const session=new WatchSession(new ProjectionStore('b_x',()=>{}),async function*(){calls++});
  const run=session.start();await tick();session.stop();await run;assert.equal(calls,1);
});
test('healthy heartbeat on resumed stream exposes live status',async()=>{
  const store=new ProjectionStore('b_x',()=>{});store.apply(begin(1n));store.apply(commit(1n));let session;const states=[];
  session=new WatchSession(store,async function*(){yield frame({kind:'heartbeat',serverRevision:1n});session.stop()},{status:s=>states.push(s)});
  await session.start();assert.ok(states.includes('live'));assert.equal(store.resume().revision,1n);
});
