import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import fs from 'node:fs';
import os from 'node:os';
import { fileURLToPath } from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const protoc=process.env.PROTOC ?? 'protoc';
const source='proto/kanban/framework/v1/board.proto';
const run=(mode,type,input)=>spawnSync(protoc,['-I','proto',`--${mode}=kanban.framework.v1.${type}`,source],{cwd:root,input});
function roundtrip(type,text){const encoded=run('encode',type,text);assert.equal(encoded.status,0,encoded.stderr.toString());const decoded=run('decode',type,encoded.stdout);assert.equal(decoded.status,0,decoded.stderr.toString());return decoded.stdout.toString()}
test('protoc compiles descriptor containing named services',()=>{
  const dir=fs.mkdtempSync(path.join(os.tmpdir(),'kanban-proto-'));
  try {
    const descriptor=path.join(dir,'descriptor.pb');
    const p=spawnSync(protoc,['-I','proto','--descriptor_set_out='+descriptor,source],{cwd:root});
    assert.equal(p.status,0,p.stderr.toString());
    const bytes=fs.readFileSync(descriptor);
    for(const method of ['GetBoard','WatchBoard','UpdateTaskTitle'])assert.ok(bytes.includes(Buffer.from(method)));
  } finally { fs.rmSync(dir,{recursive:true,force:true}); }
});
test('uint64 resume cursor round trips above JS safe integer',()=>{const d=roundtrip('WatchBoardRequest','board_id:"b_x" resume { epoch:"e1" scope:"board:b_x:cards:v1" revision:9007199254740993 } protocol_version:1');assert.match(d,/9007199254740993/)});
test('protobuf maximum uint64 and minimum int64 are preserved',()=>{const d=roundtrip('TaskCard','id:"t_x" title:"A" status:TASK_STATUS_TODO seq:18446744073709551615 position:-9223372036854775808');assert.match(d,/18446744073709551615/);assert.match(d,/-9223372036854775808/)});
test('expected version zero retains message presence',()=>{const present=roundtrip('UpdateTaskTitleRequest','expected {value:0}');const absent=roundtrip('UpdateTaskTitleRequest','');assert.match(present,/expected/);assert.doesNotMatch(absent,/expected/)});
test('text format encoder rejects simultaneous snapshot and delta',()=>{const encoded=run('encode','BoardFrame','snapshot_begin {revision:1 count:0} delta {base_revision:1 revision:2}');assert.notEqual(encoded.status,0)});
test('stream snapshot commit round trips with exact counts',()=>{const d=roundtrip('BoardFrame','board_id:"b_x" epoch:"e1" scope:"board:b_x:cards:v1" snapshot_commit {revision:7 count:66 chunks:2}');assert.match(d,/revision: 7/);assert.match(d,/count: 66/);assert.match(d,/chunks: 2/)});
test('Atlas refresh service compiles with exact named endpoint',()=>{const d=roundtrip('WatchChangesRequest','board_id:"b_project" protocol_version:1');assert.match(d,/b_project/)});
test('refresh sequence is not coerced to JS safe integer',()=>{const d=roundtrip('WorkspaceChangeFrame','board_id:"b_project" epoch:"e" sequence:9007199254740993 invalidated {reason:REFRESH_REASON_WRITE_HINT}');assert.match(d,/9007199254740993/)});
test('refresh and heartbeat cannot share the same oneof',()=>{const encoded=run('encode','WorkspaceChangeFrame','invalidated {reason:REFRESH_REASON_ATTACHED} heartbeat {}');assert.notEqual(encoded.status,0)});
test('heartbeat empty message preserves presence',()=>{const d=roundtrip('WorkspaceChangeFrame','board_id:"b_x" epoch:"e" sequence:1 heartbeat {}');assert.match(d,/heartbeat/)});
