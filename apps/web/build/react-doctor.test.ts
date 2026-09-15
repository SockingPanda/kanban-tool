import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import process from 'node:process';
import { expect, test } from 'vitest';
import { withCurrentSourceIndex } from './react-doctor.mjs';

test('扫描索引包含新源码、排除已删除文件，并保护用户暂存内容', () => {
  const root = mkdtempSync(join(tmpdir(), 'kanban-doctor-repro-'));
  const web = join(root, 'apps/web');
  const git = (...args: string[]) => execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();
  try {
    mkdirSync(web, { recursive: true });
    git('init', '--quiet');
    writeFileSync(join(web, 'old.ts'), 'export const old = 1;');
    writeFileSync(join(root, 'user.txt'), '原始内容');
    git('add', 'apps', 'user.txt');
    git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.test', 'commit', '--quiet', '-m', 'fixture');
    writeFileSync(join(root, 'user.txt'), '用户已暂存的内容');
    git('add', 'user.txt');
    writeFileSync(join(root, 'user.txt'), '用户尚未暂存的内容');
    rmSync(join(web, 'old.ts'));
    writeFileSync(join(web, 'new.ts'), 'export const fresh = 2;');
    const before = readFileSync(join(root, '.git/index'));
    const status = git('status', '--porcelain');
    expect(git('ls-files', 'apps/web')).toContain('old.ts');
    withCurrentSourceIndex(web, (env: NodeJS.ProcessEnv) => {
      const files = execFileSync('git', ['ls-files', 'apps/web'], { cwd: root, env: { ...process.env, ...env }, encoding: 'utf8' });
      expect(files).toContain('new.ts');
      expect(files).not.toContain('old.ts');
      expect(env.GIT_INDEX_FILE).not.toBe(join(root, '.git/index'));
    });
    expect(readFileSync(join(root, '.git/index'))).toEqual(before);
    expect(git('status', '--porcelain')).toBe(status);
    expect(git('show', ':user.txt')).toBe('用户已暂存的内容');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
