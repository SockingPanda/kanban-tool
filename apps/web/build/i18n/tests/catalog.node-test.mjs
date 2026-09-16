import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { readJsonStrictText, inspectNamespace, placeholders, checkCatalog, accept, sync, generate, loadCatalog, structuralIssues } from '../catalog.mjs';
const shipped=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../../../src/platform/localization');
function fixture(t) { const dir=fs.mkdtempSync(path.join(os.tmpdir(),'kb-i18n-test-'));fs.cpSync(shipped,dir,{recursive:true});t.after(()=>fs.rmSync(dir,{recursive:true,force:true}));return dir; }
function edit(root,file,change) { const p=path.join(root,file);const data=JSON.parse(fs.readFileSync(p,'utf8'));change(data);fs.writeFileSync(p,JSON.stringify(data,null,2)+'\n'); }

test('shipped source and target catalogs, provenance and generated files pass',()=>assert.deepEqual(checkCatalog(shipped).errors,[]));
test('duplicate JSON members are rejected before JSON.parse loses evidence',()=>assert.throws(()=>readJsonStrictText('{"x":"one","x":"two"}'),/duplicate/));
test('escaped duplicate names are rejected',()=>assert.throws(()=>readJsonStrictText('{"x":"one","\\u0078":"two"}'),/duplicate/));
test('JSON comments and trailing comma are not accepted',()=>{assert.throws(()=>readJsonStrictText('{/*x*/"x":"a"}'));assert.throws(()=>readJsonStrictText('{"x":"a",}'));});
test('reserved prototype members are rejected',()=>assert.throws(()=>readJsonStrictText('{"__proto__":"x"}'),/reserved/));
test('non-string and empty translations are rejected',()=>{assert.throws(()=>inspectNamespace({x:42},'en','test'));assert.throws(()=>inspectNamespace({x:' '},'en','test'));});
test('placeholders are deduplicated and deterministic',()=>assert.deepEqual(placeholders('{{name}}: {{count}} / {{name}}'),['count','name']));
test('unsafe and unsupported formatting is rejected',()=>{for(const value of ['<script>x</script>','$t(secret)','{{- html}}','{count, plural, one {a} other {b}}','{{name, format}}'])assert.throws(()=>placeholders(value));});
test('Chinese and English plural resources have different required physical forms',()=>{
  assert.equal(inspectNamespace({total_other:'{{count}} 项'},'zh','x').size,1);
  assert.throws(()=>inspectNamespace({total_other:'{{count}} items'},'en','x'),/one/);
  assert.equal(inspectNamespace({total_one:'{{count}} item',total_other:'{{count}} items'},'en','x').size,1);
});
test('plural variants cannot change parameters',()=>assert.throws(()=>inspectNamespace({total_one:'{{count}} {{name}}',total_other:'{{count}} items'},'en','x'),/different parameters/));
test('plural variants require a numeric count contract',()=>assert.throws(()=>inspectNamespace({total_one:'item',total_other:'items'},'en','x'),/count/));
test('scalar and plural logical key cannot collide',()=>assert.throws(()=>inspectNamespace({total:'Total',total_one:'{{count}} item',total_other:'{{count}} items'},'en','x'),/collide/));
test('missing logical key fails parity',t=>{const dir=fixture(t);edit(dir,'locales/en/shell.json',d=>delete d.chooseProject);assert.ok(checkCatalog(dir).errors.some(e=>e.includes('missing key shell:chooseProject')));});
test('obsolete translated key fails parity',t=>{const dir=fixture(t);edit(dir,'locales/en/shell.json',d=>d.obsolete='Old');assert.ok(checkCatalog(dir).errors.some(e=>e.includes('obsolete key')));});
test('target cannot replace a placeholder name',t=>{const dir=fixture(t);edit(dir,'locales/en/shell.json',d=>d.archivedProject='{{project}} Archived');assert.ok(checkCatalog(dir).errors.some(e=>e.includes('placeholder names differ')));});
test('same key with changed source text invalidates the translation',t=>{const dir=fixture(t);edit(dir,'locales/zh/shell.json',d=>d.chooseProject='切换项目');assert.ok(checkCatalog(dir).errors.some(e=>e.includes('source changed')));});
test('translator context changes invalidate translation acceptance',t=>{const dir=fixture(t);edit(dir,'context.json',d=>d['shell:chooseProject']='Different meaning');assert.ok(checkCatalog(dir).errors.some(e=>e.includes('source changed')));});
test('unacknowledged target edits are detected',t=>{const dir=fixture(t);edit(dir,'locales/en/shell.json',d=>d.chooseProject='Switch project');assert.ok(checkCatalog(dir).errors.some(e=>e.includes('target changed')));});
test('acceptance requires an explicit origin and note',t=>{const dir=fixture(t);assert.throws(()=>accept(dir,'en',['shell:chooseProject'],{}),/origin/);});
test('explicit per-key acceptance repairs drift without pretending to be human reviewed',t=>{const dir=fixture(t);edit(dir,'locales/zh/shell.json',d=>d.chooseProject='切换项目');accept(dir,'en',['shell:chooseProject'],{origin:'model-assisted',note:'Target wording remains suitable for switching.'});assert.deepEqual(checkCatalog(dir).errors,[]);const record=JSON.parse(fs.readFileSync(path.join(dir,'metadata/en.json')));assert.equal(record['shell:chooseProject'].state,'translated');});
test('sync inserts blanks and never overwrites existing target or metadata',t=>{const dir=fixture(t);const meta=fs.readFileSync(path.join(dir,'metadata/en.json'),'utf8');edit(dir,'locales/zh/shell.json',d=>{d.newAction='新操作';d.chooseProject='切换项目';});sync(dir);const target=JSON.parse(fs.readFileSync(path.join(dir,'locales/en/shell.json')));assert.equal(target.newAction,'');assert.equal(target.chooseProject,'Choose a project');assert.equal(fs.readFileSync(path.join(dir,'metadata/en.json'),'utf8'),meta);assert.throws(()=>checkCatalog(dir),/empty/);});
test('sync expands English plural categories without copying source Chinese',t=>{const dir=fixture(t);edit(dir,'locales/zh/shell.json',d=>d.widgets_other='{{count}} 个控件');sync(dir);const target=JSON.parse(fs.readFileSync(path.join(dir,'locales/en/shell.json')));assert.equal(target.widgets_one,'');assert.equal(target.widgets_other,'');});
test('generation is deterministic and stale output is caught',t=>{const dir=fixture(t);generate(dir);const file=path.join(dir,'contracts.generated.ts'),before=fs.readFileSync(file,'utf8');generate(dir);assert.equal(fs.readFileSync(file,'utf8'),before);fs.appendFileSync(file,'// dirty\n');assert.ok(checkCatalog(dir).errors.some(e=>e.includes('generated output is stale')));});
test('logical parity accepts language-specific physical plural variants',()=>{const c=loadCatalog(shipped);assert.equal(c.groups.zh.get('tasks:count').plural,true);assert.deepEqual([...c.groups.zh.keys()].sort(),[...c.groups.en.keys()].sort());assert.deepEqual(structuralIssues(c),[]);});
