import test from 'node:test';
import assert from 'node:assert/strict';
import { ChangeSequence, createChangeRealtime } from '../dist/changes.js';
import { atlasRpcEndpoint } from '../dist/endpoint.js';
const frame=(sequence=1n,reason='attached',extra={})=>({boardId:'b_x',epoch:'epoch',sequence,kind:'refresh',reason,...extra});
const heartbeat=(sequence=1n)=>({boardId:'b_x',epoch:'epoch',sequence,kind:'heartbeat'});
const turn=()=>new Promise(r=>setImmediate(r));
async function eventually(fn){for(let i=0;i<100;i++){if(fn())return;await new Promise(r=>setTimeout(r,2))}assert.ok(fn(),'condition timed out')}
function context(overrides={}){return {boardId:'b_x',boardSelector:'project',onRefresh(){},onState(){},...overrides}}
test('refresh starts with attached and monotonic writes',()=>{const s=new ChangeSequence('b_x');assert.equal(s.accept(frame()),true);assert.equal(s.accept(frame(2n,'write_hint')),true)});
test('heartbeat does not create a query refresh',()=>{const s=new ChangeSequence('b_x');s.accept(frame());assert.equal(s.accept(heartbeat()),false);s.accept(frame(2n,'write_hint'));assert.equal(s.accept(heartbeat(2n)),false)});
test('heartbeat cannot establish an initial baseline',()=>assert.throws(()=>new ChangeSequence('b_x').accept(heartbeat())));
test('initial write hint cannot replace attached',()=>assert.throws(()=>new ChangeSequence('b_x').accept(frame(1n,'write_hint'))));
test('initial sequence must be one',()=>assert.throws(()=>new ChangeSequence('b_x').accept(frame(7n))));
test('wrong board is rejected',()=>assert.throws(()=>new ChangeSequence('b_x').accept(frame(1n,'attached',{boardId:'b_y'}))));
test('epoch cannot change within one RPC',()=>{const s=new ChangeSequence('b_x');s.accept(frame());assert.throws(()=>s.accept(frame(2n,'write_hint',{epoch:'other'})))});
test('duplicate refresh is rejected rather than silently acknowledged',()=>{const s=new ChangeSequence('b_x');s.accept(frame());assert.throws(()=>s.accept(frame()))});
test('sequence gaps force a new connection',()=>{const s=new ChangeSequence('b_x');s.accept(frame());assert.throws(()=>s.accept(frame(3n,'write_hint')))});
test('heartbeat cannot advance query refresh sequence',()=>{const s=new ChangeSequence('b_x');s.accept(frame());assert.throws(()=>s.accept(heartbeat(2n)))});
test('heartbeat may not carry write reason',()=>{const s=new ChangeSequence('b_x');s.accept(frame());assert.throws(()=>s.accept({...heartbeat(),reason:'write_hint'}))});
test('unknown kind and reason are rejected',()=>{for(const value of [frame(1n,'future'),frame(1n,'attached',{kind:'future'})])assert.throws(()=>new ChangeSequence('b_x').accept(value))});
test('invalid uint64 and oversized epoch are rejected',()=>{for(const value of [frame(0n),frame(18446744073709551616n),frame(1),frame(1n,'attached',{epoch:'x'.repeat(129)})])assert.throws(()=>new ChangeSequence('b_x').accept(value))});
test('reconnection accepts a new epoch and forces fresh queries',()=>{const a=new ChangeSequence('b_x'),b=new ChangeSequence('b_x');a.accept(frame());assert.equal(b.accept(frame(1n,'attached',{epoch:'new'})),true)});
test('same-origin rpc prefix is preserved',()=>assert.equal(atlasRpcEndpoint('/rpc/','http://127.0.0.1:8721/app/'),'http://127.0.0.1:8721/rpc'));
test('localhost aliases and different ports are not treated as same origin',()=>{for(const url of ['http://localhost:8721','http://127.0.0.1:8722'])assert.throws(()=>atlasRpcEndpoint(url,'http://127.0.0.1:8721/app/'))});
test('public endpoints, credentials, query, fragment, and dot segments are rejected',()=>{for(const url of ['https://example.com','http://user:pw@127.0.0.1:8721','/rpc?a=1','/rpc#x','/x/../rpc','/%2e%2e/rpc','/rpc%2fsecret','/rpc\\secret'])assert.throws(()=>atlasRpcEndpoint(url,'http://127.0.0.1:8721/app/'))});
test('invalid controller configuration is rejected',()=>{for(const options of [{livenessMs:0},{livenessMs:Infinity},{maxFailures:0},{maxFailures:0.5}])assert.throws(()=>createChangeRealtime('x',async function*(){},options))});
test('start is idempotent and refresh is separate from heartbeat',async()=>{
 let opens=0,refreshes=0;let release;
 const pending=new Promise(r=>release=r);
 const source=createChangeRealtime('rpc',async function*(_b,signal){opens++;yield frame();yield heartbeat();await pending},{livenessMs:10000});
 const c=source.create(context({onRefresh(){refreshes++}}));c.start();c.start();await eventually(()=>refreshes===1);assert.equal(opens,1);assert.equal(c.snapshot().state,'live');c.stop();release();await turn();assert.equal(c.snapshot().state,'stopped');
});
test('every reconnect requests an attached refresh without an audit cursor',async()=>{
 let opens=0,refreshes=0;
 const source=createChangeRealtime('rpc',async function*(board,signal){assert.equal(board,'b_x');assert.ok(signal instanceof AbortSignal);opens++;yield frame(1n,'attached',{epoch:String(opens)})},{maxFailures:3,delay:async()=>{},livenessMs:1000});
 const c=source.create(context({onRefresh(){refreshes++}}));c.start();await eventually(()=>c.snapshot().state==='failed');assert.equal(opens,3);assert.equal(refreshes,3);c.stop();
});
test('terminal protocol mismatch never falls back to SSE or repeats',async()=>{
 let opens=0;const problem=new Error('unimplemented');const source=createChangeRealtime('rpc',async function*(){opens++;throw problem},{terminal:e=>e===problem});
 const c=source.create(context());c.start();await eventually(()=>c.snapshot().state==='failed');assert.equal(opens,1);c.stop();
});
test('liveness watchdog can stop a noncooperative iterator',async()=>{
 let opens=0;const source=createChangeRealtime('rpc',()=>{opens++;return {[Symbol.asyncIterator](){return {next:()=>new Promise(()=>{}),return:()=>new Promise(()=>{})}}}},{livenessMs:5,maxFailures:2,delay:async()=>{}});
 const c=source.create(context());c.start();await eventually(()=>c.snapshot().state==='failed');assert.equal(opens,2);c.stop();
});
test('retired stream cannot refresh after a manual restart',async()=>{
 let first,opens=0,refreshes=0;const source=createChangeRealtime('rpc',()=>{const n=++opens;return {[Symbol.asyncIterator](){return {next:()=>n===1?new Promise(r=>first=r):new Promise(()=>{}),return:async()=>({done:true})}}}},{livenessMs:10000});
 const c=source.create(context({onRefresh(){refreshes++}}));c.start();await eventually(()=>first!==undefined);c.retry();first({done:false,value:frame()});await turn();assert.equal(refreshes,0);assert.equal(opens,2);c.stop();
});
test('stop inside refresh callback does not report live afterwards',async()=>{
 const states=[];const source=createChangeRealtime('rpc',async function*(){yield frame()});let c;c=source.create(context({onRefresh(){c.stop()},onState:s=>states.push(s)}));c.start();await eventually(()=>states.includes('stopped'));assert.equal(states.includes('live'),false);assert.equal(c.snapshot().state,'stopped');
});
test('stop while waiting retry prevents another connection',async()=>{
 let opens=0,inDelay=false;const source=createChangeRealtime('rpc',async function*(){opens++},{delay:async(_ms,signal)=>{inDelay=true;await new Promise(r=>signal.addEventListener('abort',r,{once:true}))}});
 const c=source.create(context());c.start();await eventually(()=>inDelay);c.stop();await turn();assert.equal(opens,1);
});
