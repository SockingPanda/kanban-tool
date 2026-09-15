#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const checks = [];
function walk(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    if (['node_modules', 'target', 'dist', '.git'].includes(entry.name)) return [];
    const file = path.join(dir, entry.name);
    if (entry.isSymbolicLink()) throw new Error(`不打包符号链接: ${file}`);
    return entry.isDirectory() ? walk(file) : [file];
  });
}
function check(name, run) {
  try { const detail = run(); checks.push({ name, status: 'passed', detail }); }
  catch (error) { checks.push({ name, status: 'failed', error: String(error) }); }
}
const files = walk(root);
check('local_markdown_links', () => {
  let count = 0;
  for (const file of files.filter(p => p.endsWith('.md'))) {
    const source = fs.readFileSync(file, 'utf8');
    for (const [, link] of source.matchAll(/\[[^\]]*\]\(([^)\s]+)\)/g)) {
      if (/^[a-z][a-z\d+.-]*:/i.test(link) || link.startsWith('#')) continue;
      const target = path.resolve(path.dirname(file), decodeURIComponent(link.split('#')[0]));
      assert(fs.existsSync(target), `${path.relative(root, file)}: ${link}`);
      count++;
    }
  }
  return { count };
});
check('task_graph', () => {
  const plan = JSON.parse(fs.readFileSync(path.join(root, 'tasks/tasks.json'), 'utf8'));
  assert.equal(plan.directlyImportable, false);
  assert.equal(plan.tasks.length, 9);
  assert.equal(plan.baseBranch, 'codex/v4-atlas-paper');
  assert.equal(plan.baseCommit, 'f53e7b884b440f782020587c6f08a7e451757484');
  const map = new Map(plan.tasks.map(t => [t.id, t]));
  assert.equal(map.size, plan.tasks.length);
  const seen = new Set(), active = new Set();
  function visit(id) {
    assert(map.has(id), `不存在的任务: ${id}`);
    assert(!active.has(id), `依赖环: ${id}`);
    if (seen.has(id)) return;
    active.add(id);
    for (const dep of map.get(id).dependencies) visit(dep);
    active.delete(id); seen.add(id);
  }
  for (const task of plan.tasks) {
    visit(task.id);
    for (const key of ['steps', 'acceptance', 'inputs', 'outputs', 'nonGoals']) assert(task[key]?.length, `${task.id}.${key}`);
    for (const input of task.inputs) assert(fs.existsSync(path.join(root, input)), `${task.id} 输入缺失: ${input}`);
  }
  return { tasks: map.size, acyclic: true };
});
check('rpc_inventory', () => {
  const proto = fs.readFileSync(path.join(root, 'proto/kanban/framework/v1/board.proto'), 'utf8');
  const methods = [...proto.matchAll(/\brpc\s+(\w+)\s*\(/g)].map(m => m[1]);
  assert.deepEqual(methods, ['GetBoard', 'WatchBoard', 'UpdateTaskTitle', 'WatchChanges']);
  const handler = fs.readFileSync(path.join(root, 'crates/kanban-rpc-host/src/service.rs'), 'utf8') + fs.readFileSync(path.join(root, 'crates/kanban-rpc-host/src/refresh.rs'), 'utf8');
  for (const method of ['get_board', 'watch_board', 'update_task_title', 'watch_changes']) assert(handler.includes(`async fn ${method}(`));
  for (const file of files.filter(p => /\.(rs|ts|proto)$/.test(p) && !p.includes('/tests/') && !p.endsWith('/tests.rs'))) {
    const text = fs.readFileSync(file, 'utf8');
    assert(!/\b(?:todo|unimplemented)!\s*\(/.test(text), `${file}: 占位实现`);
  }
  return { methods, services: [...proto.matchAll(/\bservice\s+\w+\s*\{/g)].length };
});
check('service_patch_plan', () => {
  const plan = JSON.parse(fs.readFileSync(path.join(root, 'integration/patch-plan.json'), 'utf8'));
  assert.equal(plan.baseCommit, 'f53e7b884b440f782020587c6f08a7e451757484');
  assert.equal(plan.requiredPackagePath, 'migration/grpc-framework');
  assert.equal(plan.files.length, 10); assert.equal(plan.additions.length, 5);
  const paths = new Set();
  for (const file of [...plan.files, ...plan.additions]) {
    assert(/^(crates\/kanban-(service|server)\/|apps\/web\/src\/application\/)/.test(file.path));
    assert(!/\/(styles|components|features)\//.test(file.path));
    assert(!paths.has(file.path)); paths.add(file.path);
    assert(!file.path.split('/').includes('..'));
  }
  for (const file of plan.files) {
    assert(/^[a-f0-9]{40}$/.test(file.blob));
    for (const edit of file.edits) {
      assert(edit.before.length > 0 && edit.after.length > 0);
      assert(Number.isSafeInteger(edit.count) && edit.count > 0);
    }
  }
  for (const file of plan.additions) assert(fs.existsSync(path.join(root, file.source)));
  return { modifiedFiles: plan.files.length, addedFiles: plan.additions.length };
});
check('javascript_syntax', () => {
  const scripts = files.filter(file => file.endsWith('.mjs'));
  for (const file of scripts) {
    const result = spawnSync(process.execPath, ['--check', file], { encoding: 'utf8' });
    assert.equal(result.status, 0, `${file}: ${result.stderr}`);
  }
  return { files: scripts.length };
});
check('offline_test_receipts', () => {
  const results = JSON.parse(fs.readFileSync(path.join(root, 'validation/offline-results.json'), 'utf8'));
  for (const result of results) assert.equal(result.status, 'passed', result.name);
  const counts = [
    ['web-tests.tap', 68],
    ['installer-tests.tap', 13],
    ['protobuf-tests.tap', 10],
    ['atlas-tests.tap', 14],
  ];
  for (const [name, expected] of counts) {
    const log = fs.readFileSync(path.join(root, 'validation', name), 'utf8');
    assert(log.includes(`# pass ${expected}`), `${name}: 通过数量不符`);
    assert(log.includes('# fail 0'), `${name}: 存在失败`);
  }
  return { passedTests: counts.reduce((sum, [, value]) => sum + value, 0) };
});
fs.writeFileSync(path.join(root, 'validation/package-check.json'), JSON.stringify(checks, null, 2) + '\n');
for (const check of checks) console.log(`${check.name}: ${check.status}${check.error ? ' ' + check.error : ''}`);
if (checks.some(check => check.status !== 'passed')) process.exitCode = 1;
