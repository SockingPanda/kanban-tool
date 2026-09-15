#!/usr/bin/env node
// 离线检查只证明明确列出的范围；Rust、完整 Atlas 与生成客户端联编另行执行。
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const localTsc=path.join(root,'web/node_modules/.bin/tsc');
const steps=[
 ['TypeScript core typecheck',fs.existsSync(localTsc)?localTsc:'tsc',['-p','web/tsconfig.core.json'],'typescript-core.log',null],
 ['Frontend behavior tests',process.execPath,['--test',...fs.readdirSync(path.join(root,'web/tests')).filter(n=>n.endsWith('.test.mjs')).sort().map(n=>'web/tests/'+n)],'web-tests.tap',68],
 ['Installer fixture tests',process.execPath,['--test','tests/apply.test.mjs'],'installer-tests.tap',13],
 ['Protobuf compiler tests',process.execPath,['--test','tests/proto.test.mjs'],'protobuf-tests.tap',10],
 ['Atlas wiring fixtures',process.execPath,['--test','tests/atlas.test.mjs'],'atlas-tests.tap',14],
];
fs.mkdirSync(path.join(root,'validation'),{recursive:true});
const results=[];
for(const [name,cmd,args,file,expected] of steps){
 const out=spawnSync(cmd,args,{cwd:root,env:process.env,encoding:'utf8',maxBuffer:16*1024*1024});
 const log=(out.stdout??'')+(out.stderr??'')+(out.error?String(out.error):'');
 fs.writeFileSync(path.join(root,'validation',file),log);
 const count=expected===null?null:Number(log.match(/^# pass (\d+)$/m)?.[1]??NaN);
 const failed=expected===null?null:Number(log.match(/^# fail (\d+)$/m)?.[1]??NaN);
 const ok=out.status===0 && (expected===null || count===expected&&failed===0);
 results.push({name,command:[cmd,...args],status:ok?'passed':'failed',exitCode:out.status,log:file,passedTests:count,failedTests:failed});
 console.log(`${name}: ${ok?'PASS':'FAIL'}${count===null?'':` (${count})`}`);
}
fs.writeFileSync(path.join(root,'validation/offline-results.json'),JSON.stringify(results,null,2)+'\n');
if(results.some(x=>x.status!=='passed'))process.exitCode=1;
