import test from 'node:test';
import assert from 'node:assert/strict';
import { ProjectionStore } from '../dist/reducer.js';
import { ProtocolError, U64_MAX, orderedCards } from '../dist/model.js';
const card = (id='t_a', overrides={}) => ({id,title:'A',status:2,priority:1,position:0n,seq:1n,lockVersion:0n,...overrides});
const frame = (body, overrides={}) => ({boardId:'b_x',epoch:'e1',scope:'board:b_x:cards:v1',body,...overrides});
function fixture(limits) { const published=[]; return {published,store:new ProjectionStore('b_x', v=>published.push(v),limits)} }
function seed(store, tasks=[card()], revision=1n, epoch='e1') {
  store.apply(frame({kind:'begin',revision,count:tasks.length},{epoch}));
  if(tasks.length) store.apply(frame({kind:'chunk',index:0,tasks},{epoch}));
  store.apply(frame({kind:'commit',revision,count:tasks.length,chunks:tasks.length?1:0},{epoch}));
}
const delta = (overrides={}) => frame({kind:'delta',base:1n,revision:2n,upserts:[card('t_a',{title:'B',lockVersion:1n})],removed:[],...overrides});

test('snapshot remains invisible until complete commit',()=>{
  const {store,published}=fixture();
  store.apply(frame({kind:'begin',revision:1n,count:1}));
  store.apply(frame({kind:'chunk',index:0,tasks:[card()]}));
  assert.equal(store.resume(),undefined); assert.equal(published.length,0);
  store.apply(frame({kind:'commit',revision:1n,count:1,chunks:1}));
  assert.equal(published.length,1); assert.equal(store.resume().revision,1n);
});
test('empty snapshot commits with zero chunks',()=>{const {store}=fixture();seed(store,[]);assert.equal(store.snapshot().cards.size,0)});
test('cross-board frames are rejected',()=>{const {store}=fixture();assert.throws(()=>store.apply(frame({kind:'begin',revision:1n,count:0},{boardId:'b_y'})),ProtocolError)});
test('cross-query scope is rejected',()=>{const {store}=fixture();assert.throws(()=>store.apply(frame({kind:'heartbeat',serverRevision:0n},{scope:'other'})),ProtocolError)});
test('chunk without begin is rejected',()=>{const {store}=fixture();assert.throws(()=>store.apply(frame({kind:'chunk',index:0,tasks:[card()]})),ProtocolError)});
test('out-of-order chunks are rejected',()=>{const {store}=fixture();store.apply(frame({kind:'begin',revision:1n,count:1}));assert.throws(()=>store.apply(frame({kind:'chunk',index:1,tasks:[card()]})),ProtocolError)});
test('duplicate IDs within chunk are rejected atomically',()=>{
  const {store}=fixture();store.apply(frame({kind:'begin',revision:1n,count:1}));
  assert.throws(()=>store.apply(frame({kind:'chunk',index:0,tasks:[card(),card()]})),ProtocolError);
  store.apply(frame({kind:'chunk',index:0,tasks:[card()]}));store.apply(frame({kind:'commit',revision:1n,count:1,chunks:1}));assert.equal(store.snapshot().cards.size,1);
});
test('incomplete commit does not expose partial snapshot',()=>{const {store}=fixture();store.apply(frame({kind:'begin',revision:1n,count:1}));assert.throws(()=>store.apply(frame({kind:'commit',revision:1n,count:1,chunks:0})),ProtocolError);assert.equal(store.resume(),undefined)});
test('duplicate begin is rejected',()=>{const {store}=fixture();const f=frame({kind:'begin',revision:1n,count:0});store.apply(f);assert.throws(()=>store.apply(f),ProtocolError)});
test('reset preserves old visible view but disables resume',()=>{const {store}=fixture();seed(store);store.apply(frame({kind:'reset',reason:'history_expired'}));assert.equal(store.resume(),undefined);assert.equal(store.snapshot().cards.size,1)});
test('new epoch needs explicit reset before replacement',()=>{const {store}=fixture();seed(store);assert.throws(()=>seed(store,[],1n,'e2'),ProtocolError);store.requireSnapshot();seed(store,[],1n,'e2');assert.equal(store.resume().epoch,'e2')});
test('uint64 above JS safe integer is lossless',()=>{const {store}=fixture();const rev=9007199254740993n;seed(store,[card()],rev);store.apply(delta({base:rev,revision:rev+1n}));assert.equal(store.resume().revision,rev+1n)});
test('uint64 overflow is rejected',()=>{const {store}=fixture();assert.throws(()=>seed(store,[],U64_MAX+1n),ProtocolError)});
test('one delta publishes upserts and removals atomically',()=>{const {store,published}=fixture();seed(store,[card(),card('t_b')]);store.apply(delta({removed:['t_b']}));assert.equal(published.length,2);assert.equal(store.snapshot().cards.size,1);assert.equal(store.snapshot().cards.get('t_a').title,'B')});
test('identical duplicate delta is idempotent',()=>{const {store,published}=fixture();seed(store);store.apply(delta());store.apply(delta());assert.equal(published.length,2)});
test('same revision with different payload fails closed',()=>{const {store}=fixture();seed(store);store.apply(delta());assert.throws(()=>store.apply(delta({upserts:[card('t_a',{title:'different'})]})),ProtocolError)});
test('delta cannot skip a missing revision',()=>{const {store}=fixture();seed(store);assert.throws(()=>store.apply(delta({base:3n,revision:4n})),ProtocolError);assert.equal(store.resume().revision,1n)});
test('delta must move exactly one projection revision',()=>{const {store}=fixture();seed(store);assert.throws(()=>store.apply(delta({revision:5n})),ProtocolError)});
test('stale lock version cannot replace new task data',()=>{const {store}=fixture();seed(store,[card('t_a',{lockVersion:9n})]);assert.throws(()=>store.apply(delta()),ProtocolError)});
test('unknown removals indicate an inconsistent baseline',()=>{const {store}=fixture();seed(store);assert.throws(()=>store.apply(delta({removed:['t_missing']})),ProtocolError)});
test('same ID cannot be removed and upserted in a batch',()=>{const {store}=fixture();seed(store);assert.throws(()=>store.apply(delta({removed:['t_a']})),ProtocolError)});
test('heartbeat never confirms unapplied server revision',()=>{const {store}=fixture();seed(store);store.apply(frame({kind:'heartbeat',serverRevision:100n}));assert.equal(store.resume().revision,1n)});
test('delta from previous epoch is rejected',()=>{const {store}=fixture();seed(store);assert.throws(()=>store.apply({...delta(),epoch:'e0'}),ProtocolError)});
test('disconnect discards staging and keeps confirmed cursor',()=>{const {store}=fixture();seed(store);store.apply(frame({kind:'begin',revision:2n,count:1}));store.discardPending();assert.equal(store.resume().revision,1n)});
test('publication failure does not acknowledge the delta',()=>{let fail=false;const store=new ProjectionStore('b_x',()=>{if(fail)throw Error('publish failed')});seed(store);fail=true;assert.throws(()=>store.apply(delta()),/publish failed/);assert.equal(store.resume().revision,1n)});
test('snapshot byte budget rejects before publication',()=>{const {store}=fixture({maxCards:10,maxBytes:1});store.apply(frame({kind:'begin',revision:1n,count:1}));assert.throws(()=>store.apply(frame({kind:'chunk',index:0,tasks:[card()]})),ProtocolError);assert.equal(store.resume(),undefined)});
test('snapshot count budget is enforced',()=>{const {store}=fixture({maxCards:1,maxBytes:9999});assert.throws(()=>store.apply(frame({kind:'begin',revision:1n,count:2})),ProtocolError)});
test('unknown task status is not silently coerced',()=>{const {store}=fixture();assert.throws(()=>seed(store,[card('t_a',{status:99})]),ProtocolError)});
test('empty titles are rejected',()=>{const {store}=fixture();assert.throws(()=>seed(store,[card('t_a',{title:''})]),ProtocolError)});
test('same-epoch snapshot cannot rewind published state',()=>{const {store}=fixture();seed(store,[],10n);assert.throws(()=>seed(store,[],9n),ProtocolError)});
test('unchanged task retains reference across delta',()=>{const {store}=fixture();seed(store,[card(),card('t_b')]);const old=store.snapshot().cards.get('t_b');store.apply(delta());assert.equal(store.snapshot().cards.get('t_b'),old)});
test('ordering is deterministic without unsafe bigint coercion',()=>{const {store}=fixture();seed(store,[card('t_b'),card('t_a'),card('t_c',{position:9223372036854775807n})]);assert.deepEqual(orderedCards(store.snapshot()).map(x=>x.id),['t_a','t_b','t_c'])});
test('unknown required frame cannot move cursor',()=>{const {store}=fixture();seed(store);assert.throws(()=>store.apply(frame({kind:'future'})),ProtocolError);assert.equal(store.resume().revision,1n)});
test('randomized sequence of 1000 batches matches reference map',()=>{
  const {store}=fixture();seed(store,[]);const expected=new Map();let state=12345;let rev=1n;
  for(let i=0;i<1000;i++){
    state=(Math.imul(state,1664525)+1013904223)>>>0;const id=`t_${state%25}`;const remove=(state&8)!==0&&expected.has(id);
    const next=card(id,{title:`v${i}`,lockVersion:BigInt(i+1),position:BigInt(state%7)});
    store.apply(delta({base:rev,revision:rev+1n,upserts:remove?[]:[next],removed:remove?[id]:[]}));rev++;
    if(remove)expected.delete(id);else expected.set(id,next);
    assert.deepEqual([...store.snapshot().cards].sort(),[...expected].sort());assert.equal(store.resume().revision,rev);
  }
});
