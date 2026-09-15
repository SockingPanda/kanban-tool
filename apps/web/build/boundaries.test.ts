import { describe, expect, test } from 'vitest';
import { checkBoundaries, readSources } from './boundaries';

describe('前端分层边界', () => {
  test('产品源码遵守分层、公开接口和 I/O 边界', () => {
    expect(checkBoundaries(readSources(new URL('../src', import.meta.url).pathname))).toEqual([]);
  });
  test('能拒绝私有导入、反向依赖、循环和绕过数据源的读取', () => {
    const files = new Map([
      ['features/tasks/view.ts', "import '../health/private'; fetch('/api/tasks'); localStorage.setItem('task','x');"],
      ['features/health/private.ts', "import '../../domain/model';"],
      ['domain/model.ts', "import '../features/tasks/view';"],
    ]);
    const issues = checkBoundaries(files).join('\n');
    expect(issues).toContain('跨功能');
    expect(issues).toContain('反向依赖');
    expect(issues).toContain('循环引用');
    expect(issues).toContain('直接读写网络或存储');
  });
  test('公开 feature 出口可以组合自身的私有实现', () => {
    expect(checkBoundaries(new Map([
      ['app/shell.ts', "import '../features/tasks';"],
      ['features/tasks/index.ts', "export { View } from './view';"],
      ['features/tasks/view.ts', 'export const View = {};'],
    ]))).toEqual([]);
  });
});
