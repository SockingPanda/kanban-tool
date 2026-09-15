#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

export function blob(bytes) { return crypto.createHash('sha1').update(`blob ${bytes.length}\0`).update(bytes).digest('hex'); }
function safePath(root, relative) {
  if (path.isAbsolute(relative) || relative.split(/[\\/]/).some(s => s === '..' || s === '.')) throw new Error(`不安全路径: ${relative}`);
  let current = root;
  for (const part of relative.split('/')) {
    current = path.join(current, part);
    if (fs.existsSync(current) || (() => { try { fs.lstatSync(current); return true; } catch { return false; } })()) {
      if (fs.lstatSync(current).isSymbolicLink()) throw new Error(`拒绝 symlink: ${relative}`);
    }
  }
  return current;
}
export function planChanges(repo, packageRoot, manifest) {
  repo = fs.realpathSync(repo);
  const head = execFileSync('git', ['-C', repo, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  if (head !== manifest.baseCommit) throw new Error(`HEAD 不匹配: ${head}`);
  if (manifest.requiredPackagePath && fs.realpathSync(packageRoot) !== path.join(repo, manifest.requiredPackagePath)) throw new Error(`请先将框架放到 ${manifest.requiredPackagePath}`);
  const targets = [...manifest.files, ...manifest.additions].map(entry => entry.path);
  if (new Set(targets).size !== targets.length) throw new Error("补丁包含重复目标");
  const plan = [];
  for (const entry of manifest.files) {
    const target = safePath(repo, entry.path);
    const status = execFileSync("git", ["-C", repo, "status", "--porcelain", "--", entry.path], { encoding: "utf8" }).trim();
    if (status) throw new Error(`文件已改动（含暂存区）: ${entry.path}`);
    const original = fs.readFileSync(target);
    if (blob(original) !== entry.blob) throw new Error(`文件已改动或基线不匹配: ${entry.path}`);
    let updated = original.toString('utf8');
    for (const edit of entry.edits) {
      if (!edit.before || updated.split(edit.before).length - 1 !== edit.count) throw new Error(`锚点数量不匹配: ${entry.path}`);
      updated = updated.split(edit.before).join(edit.after);
    }
    plan.push({ target, relative: entry.path, original, bytes: Buffer.from(updated) });
  }
  for (const entry of manifest.additions) {
    const target = safePath(repo, entry.path);
    if (fs.existsSync(target)) throw new Error(`新增目标已存在: ${entry.path}`);
    plan.push({ target, relative: entry.path, original: null, bytes: fs.readFileSync(safePath(packageRoot, entry.source)) });
  }
  return plan;
}
export function writeChanges(plan) {
  const completed = [];
  try {
    for (const entry of plan) {
      if (entry.original === null ? fs.existsSync(entry.target) : !fs.readFileSync(entry.target).equals(entry.original)) throw new Error(`应用前目标再次改变: ${entry.relative}`);
      fs.mkdirSync(path.dirname(entry.target), { recursive: true });
      if (entry.original === null) fs.writeFileSync(entry.target, entry.bytes, { flag: 'wx' });
      else fs.writeFileSync(entry.target, entry.bytes);
      completed.push(entry);
    }
  } catch (error) {
    for (const entry of completed.reverse()) {
      // 协作环境的尽力回退；若另一个编辑器已改写则保留其内容。
      if (!fs.existsSync(entry.target) || !fs.readFileSync(entry.target).equals(entry.bytes)) continue;
      if (entry.original === null) fs.unlinkSync(entry.target); else fs.writeFileSync(entry.target, entry.original);
    }
    throw error;
  }
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [, , repo, mode] = process.argv;
    if (!repo || !['--check', '--write'].includes(mode)) throw new Error('用法: node integration/apply-atlas.mjs /path/to/kanban-tool --check|--write');
    const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
    const manifest = JSON.parse(fs.readFileSync(path.join(root, 'integration/patch-plan.json'), 'utf8'));
    const plan = planChanges(repo, root, manifest);
    if (mode === '--write') writeChanges(plan);
    console.log(JSON.stringify({ mode, base: manifest.baseCommit, files: plan.map(p => p.relative), databaseTouched: false, remoteWritten: false }, null, 2));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
