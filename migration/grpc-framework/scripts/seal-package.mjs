#!/usr/bin/env node
// 固定交付内容的哈希与验证分类。此脚本不运行测试，也不把未运行的测试标记为通过。
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const excluded = new Set(['delivery-manifest.json', 'CHECKSUMS.sha256']);
function walk(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    if (['node_modules', 'target', 'dist', '.git'].includes(entry.name)) return [];
    const file = path.join(dir, entry.name);
    if (entry.isSymbolicLink()) throw new Error('拒绝打包 symlink: ' + file);
    return entry.isDirectory() ? walk(file) : [file];
  });
}
const files = walk(root).filter(file => !excluded.has(path.relative(root, file))).sort();
const hashes = files.map(file => ({
  path: path.relative(root, file).split(path.sep).join('/'),
  bytes: fs.statSync(file).size,
  sha256: crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex'),
}));
const read = relative => fs.readFileSync(path.join(root, relative), 'utf8');
const checks = JSON.parse(read('validation/package-check.json'));
const tests = JSON.parse(read('validation/offline-results.json'));
if ([...checks, ...tests].some(check => check.status !== 'passed')) throw new Error('存在失败检查，不封存包');
const proto = read('proto/kanban/framework/v1/board.proto');
const sourceFiles = hashes.filter(file => /\.(rs|ts|mjs|proto)$/.test(file.path));
const rustTests = files.filter(file => file.endsWith('.rs')).reduce((sum, file) =>
  sum + [...fs.readFileSync(file, 'utf8').matchAll(/#\[(?:tokio::)?test(?:\([^\]]*\))?\]/g)].length, 0);
const manifest = {
  format: 'kanban.grpc-framework.delivery.v1',
  artifactKind: 'atlas_core_migration_framework',
  frameworkVersion: '0.2.0',
  baseBranch: 'codex/v4-atlas-paper',
  repository: 'SockingPanda/kanban-tool',
  baseCommit: 'f53e7b884b440f782020587c6f08a7e451757484',
  completeOriginalRepository: false,
  modifiesRemote: false,
  containsProductionDatabase: false,
  sourceFileCount: sourceFiles.length,
  rpcCount: [...proto.matchAll(/\brpc\s+\w+\s*\(/g)].length,
  rustTestSourceCount: rustTests,
  scope: {
    framework: 'provided',
    atlasWiringPatch: 'provided_not_applied_to_complete_repository',
    defaultProductionRpcActivated: false,
    atlasVisualFilesModified: false,
    fullPaginatedQueryDeltas: 'follow_up_G07',
    workspaceRefresh: 'typed_grpc_invalidation_no_sse_fallback',
    existingProductSurfaces: 'retained_for_follow_up_tasks',
    rpcExamples: 'GetBoard, WatchBoard, UpdateTaskTitle, WatchChanges',
    existingFilesModifiedByPatch: 10,
    newFilesAddedByPatch: 5,
    plannedTasks: 9,
    tasksWrittenToLiveKanban: false,
  },
  verification: {
    executedPassedTests: tests.reduce((sum, step) => sum + (step.passedTests ?? 0), 0),
    frontendSemanticTests: 68,
    installerFixtureTests: 13,
    protobufCompilerTests: 10,
    atlasWiringFixtureTests: 14,
    typescriptCoreTypecheck: 'passed',
    protobufDescriptor: 'compiled',
    rustBuild: 'not_run_toolchain_unavailable',
    rustTests: 'not_run',
    rustfmt: 'not_run',
    fullGeneratedWebClientTypecheck: 'not_run_dependencies_unavailable',
    realNativeGrpcAndGrpcWebInterop: 'not_run',
    completeAtlasIntegration: 'not_run',
    dependencyLockfiles: 'not_resolved',
  },
  files: hashes,
};
fs.writeFileSync(path.join(root, 'delivery-manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
const manifestBytes = fs.readFileSync(path.join(root, 'delivery-manifest.json'));
const all = [...hashes, { path: 'delivery-manifest.json', sha256: crypto.createHash('sha256').update(manifestBytes).digest('hex') }].sort((a,b) => a.path.localeCompare(b.path));
fs.writeFileSync(path.join(root, 'CHECKSUMS.sha256'), all.map(file => file.sha256 + '  ' + file.path).join('\n') + '\n');
console.log(JSON.stringify({ sourceFiles: sourceFiles.length, rustTests, rpcCount: manifest.rpcCount, hashedFiles: all.length }, null, 2));
