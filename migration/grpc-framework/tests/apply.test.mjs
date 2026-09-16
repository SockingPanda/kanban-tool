import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { blob, planChanges, writeChanges } from '../integration/apply-atlas.mjs';
function fixture() {
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'kanban-grpc-apply-'));
  const repo=path.join(root,'repo'),pkg=path.join(root,'pkg');fs.mkdirSync(repo);fs.mkdirSync(pkg);
  const git=(...args)=>execFileSync('git',['-C',repo,...args],{stdio:['ignore','pipe','pipe'],encoding:'utf8'}).trim();
  git('init','-q');fs.writeFileSync(path.join(repo,'owned.rs'),'mod old;\n');git('add','.');git('-c','user.name=FrameworkFixture','-c','user.email=fixture@invalid.example','commit','-qm','fixture');
  fs.writeFileSync(path.join(pkg,'new.rs'),'// new\n');
  const manifest={baseCommit:git('rev-parse','HEAD'),files:[{path:'owned.rs',blob:blob(Buffer.from('mod old;\n')),edits:[{before:'mod old;',after:'mod current;',count:1}]}],additions:[{source:'new.rs',path:'new.rs'}]};
  return{root,repo,pkg,manifest,cleanup:()=>fs.rmSync(root,{recursive:true,force:true})};
}
function check(name,operation){test(name,()=>{const f=fixture();try{operation(f)}finally{f.cleanup()}})}
check('dry run does not change any repository files',f=>{const plan=planChanges(f.repo,f.pkg,f.manifest);assert.equal(plan.length,2);assert.equal(fs.readFileSync(path.join(f.repo,'owned.rs'),'utf8'),'mod old;\n');assert.equal(fs.existsSync(path.join(f.repo,'new.rs')),false)});
check('writes exact planned edits and additions',f=>{writeChanges(planChanges(f.repo,f.pkg,f.manifest));assert.equal(fs.readFileSync(path.join(f.repo,'owned.rs'),'utf8'),'mod current;\n');assert.equal(fs.readFileSync(path.join(f.repo,'new.rs'),'utf8'),'// new\n')});
check('rejects wrong commit',f=>{f.manifest.baseCommit='0'.repeat(40);assert.throws(()=>planChanges(f.repo,f.pkg,f.manifest),/HEAD/)});
check('rejects dirty target',f=>{fs.writeFileSync(path.join(f.repo,'owned.rs'),'user changes');assert.throws(()=>planChanges(f.repo,f.pkg,f.manifest),/文件已改动/)});
check('preserves unrelated dirty work',f=>{fs.writeFileSync(path.join(f.repo,'notes.txt'),'user notes');writeChanges(planChanges(f.repo,f.pkg,f.manifest));assert.equal(fs.readFileSync(path.join(f.repo,'notes.txt'),'utf8'),'user notes')});
check('rejects existing addition instead of overwriting',f=>{fs.writeFileSync(path.join(f.repo,'new.rs'),'user');assert.throws(()=>planChanges(f.repo,f.pkg,f.manifest),/已存在/)});
check('rejects symlink target',f=>{fs.unlinkSync(path.join(f.repo,'owned.rs'));fs.symlinkSync(path.join(f.pkg,'new.rs'),path.join(f.repo,'owned.rs'));assert.throws(()=>planChanges(f.repo,f.pkg,f.manifest),/symlink/)});
check('rejects path traversal',f=>{f.manifest.files[0].path='../secret';assert.throws(()=>planChanges(f.repo,f.pkg,f.manifest),/不安全路径/)});
check('rejects wrong anchor counts',f=>{f.manifest.files[0].edits[0].count=2;assert.throws(()=>planChanges(f.repo,f.pkg,f.manifest),/锚点/)});
check('rechecks targets and rolls back earlier writes on conflict',f=>{const plan=planChanges(f.repo,f.pkg,f.manifest);fs.writeFileSync(path.join(f.repo,'new.rs'),'new user edit');assert.throws(()=>writeChanges(plan),/再次改变/);assert.equal(fs.readFileSync(path.join(f.repo,'owned.rs'),'utf8'),'mod old;\n');assert.equal(fs.readFileSync(path.join(f.repo,'new.rs'),'utf8'),'new user edit')});
check('rejects duplicate targets in a malformed manifest',f=>{f.manifest.files.push({...f.manifest.files[0]});assert.throws(()=>planChanges(f.repo,f.pkg,f.manifest),/重复目标/)});
check('rejects a staged target even when worktree bytes equal the baseline',f=>{fs.writeFileSync(path.join(f.repo,'owned.rs'),'staged');execFileSync('git',['-C',f.repo,'add','owned.rs']);fs.writeFileSync(path.join(f.repo,'owned.rs'),'mod old;\n');assert.throws(()=>planChanges(f.repo,f.pkg,f.manifest),/文件已改动/)});
check('requires the fixed embedded package location only when manifest requests it',f=>{f.manifest.requiredPackagePath='migration/grpc-framework';assert.throws(()=>planChanges(f.repo,f.pkg,f.manifest),/先将框架/)});
