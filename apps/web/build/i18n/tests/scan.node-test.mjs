import test from 'node:test';
import assert from 'node:assert/strict';
import { scanText, introduced, allowed } from '../scan.mjs';

test('visible JSX text and accessibility attributes are detected in both languages',()=>{
 const f=scanText('const x=<button aria-label="Create task" title="新建任务">保存</button>');
 assert.deepEqual(f.map(x=>x.text).sort(),['Create task','保存','新建任务'].sort());
});
test('dynamic translated labels, enum comparisons and comments are not display literals',()=>{
 const f=scanText('// 中文注释\n const x=<><div title={t("shell:chooseProject")}>{mode === "board" && <b>{t("common:save")}</b>}</div></>');assert.deepEqual(f,[]);
});
test('displayed string alternatives are found',()=>{assert.equal(scanText('const x=<b>{ok ? "Ready" : "Error"}</b>').length,2);});
test('raw CJK in callbacks is inventoried even outside JSX',()=>{assert.equal(scanText('function f() { notify("保存成功") }','x.ts').length,1);});
test('label metadata written in English is detected',()=>{assert.equal(scanText('const items=[{label:"Create task",value:"create"}]','x.ts').length,1);});
test('attribute values for className, route IDs and data attributes are excluded',()=>{assert.deepEqual(scanText('const x=<div className="task-list" data-testid="board" role="button" />'),[]);});
test('allowlist requires a concrete reason and exact file/text match',()=>{const f=scanText('const x=<b>atlas</b>','x.tsx');assert.throws(()=>allowed(f,[{file:'x.tsx',text:'atlas'}]),/reason/);assert.equal(allowed(f,[{file:'x.tsx',text:'atlas',reason:'Brand'}]).length,0);assert.equal(allowed(f,[{file:'y.tsx',text:'atlas',reason:'Brand'}]).length,1);});
test('line movement does not reset debt or create a false new finding',()=>{assert.deepEqual(introduced(scanText('const x=<b>Old text</b>','x.tsx'),scanText('\n\nconst x=<b>Old text</b>','x.tsx')),[]);});
test('duplicating a grandfathered literal is a regression',()=>{const before=scanText('const x=<b>Old text</b>','x.tsx'),after=scanText('const x=<><b>Old text</b><i>Old text</i></>','x.tsx');assert.equal(introduced(before,after).length,1);});
test('invalid source syntax fails closed',()=>{assert.throws(()=>scanText('const x=<div>'));});
