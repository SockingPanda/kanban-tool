import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import ts from 'typescript';

export function readSources(root: string): Map<string, string> {
  const sources = new Map<string, string>();
  function walk(directory: string) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const absolute = path.join(directory, entry.name);
      if (entry.isDirectory()) walk(absolute);
      else if (/\.tsx?$/.test(entry.name) && !/\.test\./.test(entry.name)) {
        const file = path.relative(root, absolute).replaceAll(path.sep, '/');
        if (!file.startsWith('lib/api/generated/')) sources.set(file, readFileSync(absolute, 'utf8'));
      }
    }
  }
  walk(root);
  return sources;
}

const allowed: Record<string, readonly string[]> = {
  app: ['app', 'features', 'application', 'domain', 'components', 'platform', 'lib'],
  features: ['features', 'application', 'domain', 'components', 'platform', 'lib'],
  application: ['application', 'domain', 'platform', 'lib'],
  domain: ['domain', 'lib'],
  components: ['components', 'platform', 'domain'],
  platform: ['platform', 'domain', 'lib'],
  adapters: ['adapters', 'application', 'domain', 'platform', 'lib', 'generated'],
  generated: ['generated'],
};

export function checkBoundaries(sources: ReadonlyMap<string, string>): string[] {
  const violations: string[] = [];
  const graph = new Map<string, string[]>();
  function resolve(from: string, specifier: string): string | undefined {
    if (!specifier.startsWith('.') && !specifier.startsWith('@/')) return;
    const base = specifier.startsWith('@/') ? specifier.slice(2) : path.posix.normalize(path.posix.join(path.posix.dirname(from), specifier));
    return [base, `${base}.ts`, `${base}.tsx`, `${base}/index.ts`, `${base}/index.tsx`].find(file => sources.has(file));
  }
  const permitted = new Map(Object.entries(allowed).map(([layer, targets]) => [layer, new Set(targets)]));
  for (const [file, source] of sources) {
    const layer = file.split('/')[0];
    const targetLayers = permitted.get(layer);
    const ast = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
    const dependencies = new Set<string>();
    function visit(node: ts.Node) {
      if (ts.isStringLiteral(node)) {
        const parent = node.parent;
        const moduleReference = ts.isImportDeclaration(parent) || ts.isExportDeclaration(parent) || ts.isExternalModuleReference(parent)
          || ts.isCallExpression(parent) && parent.expression.kind === ts.SyntaxKind.ImportKeyword
          || ts.isLiteralTypeNode(parent) && ts.isImportTypeNode(parent.parent);
        if (moduleReference) {
          const target = resolve(file, node.text);
          if (target) dependencies.add(target);
          if (layer === 'application' && /^(?:@bufbuild\/protobuf|@connectrpc\/)/.test(node.text)) {
            violations.push(`${file}: application 不能依赖 Protobuf 或 Connect 实现`);
          }
        }
      }
      if (['features', 'domain', 'components'].includes(layer)) {
        const expression = ts.isCallExpression(node) || ts.isNewExpression(node) ? node.expression : undefined;
        const operation = expression && (ts.isIdentifier(expression) ? expression.text : ts.isPropertyAccessExpression(expression) ? expression.name.text : undefined);
        if (operation && ['fetch', 'WebSocket', 'EventSource', 'XMLHttpRequest', 'sendBeacon'].includes(operation)
          || ts.isIdentifier(node) && ['localStorage', 'sessionStorage', 'indexedDB'].includes(node.text)) violations.push(`${file}: 功能和展示层不能直接读写网络或存储`);
      }
      ts.forEachChild(node, visit);
    }
    visit(ast);
    graph.set(file, [...dependencies]);
    for (const target of dependencies) {
      const targetLayer = target.split('/')[0];
      if (targetLayers && !targetLayers.has(targetLayer)) violations.push(`${file} -> ${target}: 反向依赖`);
      if (targetLayer === 'features' && (layer !== 'features' || target.split('/')[1] !== file.split('/')[1]) && !/^features\/[^/]+\/index\.tsx?$/.test(target)) violations.push(`${file} -> ${target}: 跨功能必须通过公开出口`);
    }
  }
  const visited = new Set<string>();
  const active = new Set<string>();
  function visit(file: string, chain: string[]) {
    if (active.has(file)) { violations.push(`循环引用: ${[...chain, file].join(' -> ')}`); return; }
    if (visited.has(file)) return;
    visited.add(file); active.add(file);
    for (const dependency of graph.get(file) ?? []) visit(dependency, [...chain, file]);
    active.delete(file);
  }
  for (const file of graph.keys()) visit(file, []);
  return [...new Set(violations)].sort();
}
