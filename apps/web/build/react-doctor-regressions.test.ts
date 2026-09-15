import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import process from 'node:process';
import { expect, test } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const web = resolve(here, '..');
const rules = new Set(['no-barrel-import', 'no-loading-flag-reset-outside-finally', 'no-pass-live-state-to-parent', 'async-await-in-loop', 'no-adjust-state-on-prop-change']);
type Diagnostic = { filePath: string; rule: string; line: number };

// 0.9.13 的五种可复现限制：显式功能出口、条件 finally、资源注册、顺序游标和异步快照增量。
// 真实调用点只豁免对应语句；此测试不改变生产扫描范围。升级 Doctor 时可据此移除失效豁免。
test('重现 Doctor 的必要异步行为误报，并验证单行规则标注', () => {
  const root = mkdtempSync(join(tmpdir(), 'kanban-doctor-rule-repro-'));
  const scan = (): Diagnostic[] => {
    const output = execFileSync(process.execPath, [join(web, 'node_modules/react-doctor/bin/react-doctor.js'), root, '--scope', 'full', '--no-score', '--no-supply-chain', '--no-parallel', '--json', '--blocking', 'none'], { encoding: 'utf8', timeout: 45_000 });
    const result = JSON.parse(output);
    expect(result.version).toBe('0.9.13');
    expect(result.ok).toBe(true);
    return result.projects.flatMap((project: { diagnostics: Diagnostic[] }) => project.diagnostics);
  };
  try {
    mkdirSync(join(root, 'feature'));
    writeFileSync(join(root, 'package.json'), JSON.stringify({ name: 'doctor-rule-repro', type: 'module', dependencies: { react: '19.2.8' } }));
    writeFileSync(join(root, 'tsconfig.json'), JSON.stringify({ compilerOptions: { jsx: 'react-jsx', target: 'ES2022', module: 'ESNext', moduleResolution: 'Bundler' }, include: ['**/*.tsx', '**/*.ts'] }));
    writeFileSync(join(root, 'feature/index.ts'), "export { Label } from './label';\nexport { unused } from './unused';\n");
    writeFileSync(join(root, 'feature/unused.ts'), 'export const unused = 1;');
    writeFileSync(join(root, 'feature/label.tsx'), 'export function Label(){return <span>label</span>;}');
    symlinkSync(join(web, 'node_modules'), join(root, 'node_modules'), 'dir');
    for (const file of ['App.tsx', 'Events.tsx']) writeFileSync(join(root, file), readFileSync(join(here, 'fixtures/react-doctor', `${file}.txt`)));
    const diagnostics = scan().filter(diagnostic => rules.has(diagnostic.rule));
    expect(new Set(diagnostics.map(diagnostic => diagnostic.rule))).toEqual(rules);
    // 只在已复现的位置插入对应规则；不使用文件级或目录级忽略。
    for (const file of ['App.tsx', 'Events.tsx']) {
      const lines = readFileSync(join(root, file), 'utf8').split('\n');
      for (const diagnostic of diagnostics.filter(item => item.filePath === file).sort((a, b) => b.line - a.line)) {
        lines.splice(diagnostic.line - 1, 0, `// react-doctor-disable-next-line react-doctor/${diagnostic.rule}`);
      }
      writeFileSync(join(root, file), lines.join('\n'));
    }
    expect(scan().filter(diagnostic => rules.has(diagnostic.rule))).toEqual([]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}, 100_000);
