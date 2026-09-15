import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath, URL } from 'node:url';

function git(cwd, env, args) {
  const result = spawnSync('git', args, { cwd, env, encoding: 'utf8' });
  if (result.status !== 0) throw new Error(result.stderr || 'Git 无法创建检查用索引。');
  return result.stdout.trim();
}

/** 0.9.13 会从 Git 索引枚举已删除的文件；临时索引只给扫描器呈现当前目录，不修改用户索引。 */
export function withCurrentSourceIndex(webRoot, run) {
  const directory = mkdtempSync(path.join(tmpdir(), 'kanban-doctor-index-'));
  const env = { ...process.env, GIT_INDEX_FILE: path.join(directory, 'index') };
  try {
    const repo = git(webRoot, env, ['rev-parse', '--show-toplevel']);
    git(repo, env, ['read-tree', 'HEAD']);
    git(repo, env, ['add', '--all', '--', path.relative(repo, webRoot)]);
    return run(env);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

export function runReactDoctor(args) {
  const [scope = 'full', base, ...extra] = args;
  if (!['full', 'changed'].includes(scope) || scope === 'changed' && !base || extra.length) throw new Error('用法：react-doctor.mjs full | changed <base>');
  const webRoot = fileURLToPath(new URL('..', import.meta.url));
  return withCurrentSourceIndex(webRoot, env => {
    const flags = ['exec', 'react-doctor', '.', '--scope', scope, '--no-score', '--no-supply-chain', '--verbose', '--blocking', 'warning'];
    if (scope === 'changed') flags.push('--base', base, '--include-untracked');
    const result = spawnSync('pnpm', flags, { cwd: webRoot, env, stdio: 'inherit' });
    if (result.error) throw result.error;
    return result.status ?? 1;
  });
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { process.exitCode = runReactDoctor(process.argv.slice(2)); }
  catch (error) { process.stderr.write(String(error) + '\n'); process.exitCode = 1; }
}
