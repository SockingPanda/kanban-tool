import assert from 'node:assert/strict';
import { Buffer } from 'node:buffer';
import { spawn, execFile } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { mkdir, mkdtemp, readFile, writeFile, access } from 'node:fs/promises';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { clearTimeout, setTimeout } from 'node:timers';
import { setTimeout as delay } from 'node:timers/promises';
import { URL } from 'node:url';
import { promisify, TextDecoder } from 'node:util';
import { chromium } from '@playwright/test';

const runFile = promisify(execFile);
const { fetch, AbortSignal } = globalThis;

/** Playwright 禁用且不支持 BFCache；此验收直接连接隔离 Chromium 的 CDP。 */
class DevTools {
  sequence = 0;
  pending = new Map();
  events = [];

  constructor(socket) {
    this.socket = socket;
    socket.addEventListener('message', ({ data }) => {
      const message = JSON.parse(data);
      if (message.id === undefined) { this.events.push(message); return; }
      const pending = this.pending.get(message.id);
      if (!pending) return;
      this.pending.delete(message.id);
      clearTimeout(pending.timer);
      if (message.error) pending.reject(new Error(JSON.stringify(message.error)));
      else pending.resolve(message.result);
    });
  }

  static async connect(url) {
    const socket = new globalThis.WebSocket(url);
    await new Promise((resolve, reject) => {
      socket.addEventListener('open', resolve, { once: true });
      socket.addEventListener('error', reject, { once: true });
    });
    return new DevTools(socket);
  }

  send(method, params = {}) {
    const id = ++this.sequence;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`CDP 超时：${method}`));
      }, 10_000);
      this.pending.set(id, { resolve, reject, timer });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  async evaluate(expression) {
    const value = await this.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (value.exceptionDetails) throw new Error(JSON.stringify(value.exceptionDetails));
    return value.result.value;
  }

  close() {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(new Error('CDP 已关闭'));
    }
    this.pending.clear();
    this.socket.close();
  }
}

async function eventually(read, label, timeout = 15_000) {
  const start = Date.now();
  let lastError;
  while (Date.now() - start < timeout) {
    try { const value = await read(); if (value) return value; }
    catch (error) { lastError = error; }
    await delay(40);
  }
  throw new Error(`等待超时：${label}`, { cause: lastError });
}

async function unusedPort() {
  const server = net.createServer();
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  return port;
}

async function hash(file) { return createHash('sha256').update(await readFile(file)).digest('hex'); }
async function alive(pid) { try { await access(`/proc/${pid}`); return true; } catch { return false; } }

function rpcRequests(events) {
  return events.flatMap(event => {
    if (event.method !== 'Network.requestWillBeSent') return [];
    const request = event.params.request;
    return new URL(request.url).pathname.startsWith('/kanban.v1.') ? [request] : [];
  });
}

// 只观察浏览器自行派发的事件，不构造 PageTransitionEvent，也不代理 fetch。
function recordLifecycle() {
  globalThis.__kanbanBfCacheProof = { documentId: globalThis.crypto.randomUUID(), events: [] };
  for (const type of ['pagehide', 'pageshow']) {
    globalThis.addEventListener(type, event => {
      globalThis.__kanbanBfCacheProof.events.push({ type, persisted: event.persisted, trusted: event.isTrusted, focus: globalThis.document.activeElement?.getAttribute('name'), target: event.target?.getAttribute?.('name') ?? event.target?.tagName ?? 'window', related: event.relatedTarget?.getAttribute?.('name') ?? event.relatedTarget?.tagName ?? null, documentFocused: globalThis.document.hasFocus(), visibility: globalThis.document.visibilityState });
    }, true);
  }
}

async function click(devtools, expression) {
  await devtools.evaluate(`(${expression}).scrollIntoView({block: 'center', inline: 'center'})`);
  const point = await eventually(() => devtools.evaluate(`(() => { const element = ${expression}; if (!element) throw new Error('控件不存在'); const rect = element.getBoundingClientRect(); const point = {x: rect.x + rect.width / 2, y: rect.y + rect.height / 2}; return element.contains(document.elementFromPoint(point.x, point.y)) ? point : null; })()`), '控件可点击');
  await devtools.send('Input.dispatchMouseEvent', { type: 'mouseMoved', ...point });
  await devtools.send('Input.dispatchMouseEvent', { type: 'mousePressed', button: 'left', clickCount: 1, ...point });
  await devtools.send('Input.dispatchMouseEvent', { type: 'mouseReleased', button: 'left', clickCount: 1, ...point });
}

async function proof(binary, webDirectory, output) {
  await mkdir(output, { recursive: true });
  await writeFile(path.join(output, 'started.json'), JSON.stringify({ startedAt: new Date().toISOString() }) + '\n', { flag: 'wx' });
  const directory = await mkdtemp(path.join(os.tmpdir(), 'kanban-bfcache-'));
  const baseUrl = `http://127.0.0.1:${await unusedPort()}`;
  const report = { binary, binarySha256: await hash(binary), webDirectory, directory, baseUrl, assertions: [], passed: false };
  let host;
  let browser;
  let devtools;
  let hostExit;
  let browserExit;
  const hostLog = [];
  const browserLog = [];
  try {
    host = spawn(binary, ['--db', path.join(directory, 'kanban.db'), '--actor', 'BFCache 验收', 'serve', '--host', '127.0.0.1', '--port', new URL(baseUrl).port, '--web-dir', webDirectory], { stdio: ['ignore', 'pipe', 'pipe'] });
    report.hostPid = host.pid;
    host.once('exit', (code, signal) => { hostExit = { code, signal }; });
    for (const stream of [host.stdout, host.stderr]) stream.on('data', bytes => hostLog.push(bytes));
    await eventually(async () => (await fetch(baseUrl + '/health', { signal: AbortSignal.timeout(500) })).ok, '隔离 Host 启动');
    const runtime = await (await fetch(baseUrl + '/app/runtime.json')).json();
    const manifestBytes = await (await fetch(baseUrl + '/app/manifest.json')).arrayBuffer();
    report.runtime = runtime;
    report.manifestSha256 = createHash('sha256').update(new Uint8Array(manifestBytes)).digest('hex');
    assert.equal(report.manifestSha256, await hash(path.join(webDirectory, 'manifest.json')));
    assert.equal(runtime.webBuildId, JSON.parse(new TextDecoder().decode(manifestBytes)).buildId);
    const board = 'bfcache-' + randomUUID().slice(0, 8);
    const cli = async (...args) => JSON.parse((await runFile(binary, ['--server-url', baseUrl, '--board', board, '--json', ...args], { timeout: 15_000 })).stdout);
    await cli('board', 'create', board, '--name', '真实 BFCache 验收');
    const task = (await cli('task', 'create', '缓存中的任务', '--status', 'todo')).data;
    report.board = board;
    report.taskId = task.id;
    const profile = path.join(directory, 'chromium');
    const executable = chromium.executablePath();
    browser = spawn(executable, ['--headless=new', '--no-sandbox', '--disable-dev-shm-usage', '--no-first-run', '--no-default-browser-check', '--remote-debugging-port=0', `--user-data-dir=${profile}`, '--window-size=1440,900', 'about:blank'], { stdio: ['ignore', 'pipe', 'pipe'] });
    report.browserPid = browser.pid;
    report.browserExecutable = executable;
    browser.once('exit', (code, signal) => { browserExit = { code, signal }; });
    for (const stream of [browser.stdout, browser.stderr]) stream.on('data', bytes => browserLog.push(bytes));
    const debuggingPort = await eventually(async () => (await readFile(path.join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0], '隔离 Chromium 启动');
    const debuggingBase = `http://127.0.0.1:${debuggingPort}`;
    const targets = await (await fetch(debuggingBase + '/json/list')).json();
    const target = targets.find(target => target.type === 'page');
    if (!target) throw new Error('隔离 Chromium 未提供页面调试目标');
    devtools = await DevTools.connect(target.webSocketDebuggerUrl);
    report.browserVersion = await devtools.send('Browser.getVersion');
    await devtools.send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
    await devtools.send('Page.enable');
    await devtools.send('Network.enable');
    await devtools.send('Page.addScriptToEvaluateOnNewDocument', { source: `(${recordLifecycle.toString()})()` });
    const url = `${baseUrl}/app/boards/${board}/list?task=${task.id}`;
    await devtools.send('Page.navigate', { url });
    await eventually(() => devtools.evaluate('document.querySelector("[name=task-title]")?.value === "缓存中的任务"'), '任务详情');
    await click(devtools, '[...document.querySelectorAll("button")].find(button => button.textContent.startsWith("讨论"))');
    await eventually(() => devtools.evaluate('document.querySelector("[data-testid=task-discussion]") !== null'), '讨论面板');
    await click(devtools, 'document.querySelector("textarea[name=comment-body]")');
    await devtools.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'a', code: 'KeyA', modifiers: 2, windowsVirtualKeyCode: 65 });
    await devtools.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'a', code: 'KeyA', modifiers: 2, windowsVirtualKeyCode: 65 });
    await devtools.send('Input.insertText', { text: 'BFCache 未保存草稿' });
    const before = await devtools.evaluate('({proof: globalThis.__kanbanBfCacheProof, draft: document.querySelector("textarea[name=comment-body]").value, focus: document.activeElement?.getAttribute("name"), buildId: document.querySelector("main")?.dataset.runtimeWebBuildId})');
    assert.equal(before.draft, 'BFCache 未保存草稿');
    assert.equal(before.focus, 'comment-body');
    assert.equal(before.buildId, runtime.webBuildId);
    const entry = (await devtools.send('Page.getNavigationHistory')).entries.find(entry => entry.url === url);
    assert(entry);
    await devtools.send('Page.navigate', { url: baseUrl + '/health' });
    await eventually(() => devtools.evaluate(`location.href === ${JSON.stringify(baseUrl + '/health')}`), '实际离开应用');
    await cli('comment', 'add', task.id, '页面冻结期间的原生更新', '--author', 'BFCache 外部写入');
    await delay(350);
    await devtools.send('Page.navigateToHistoryEntry', { entryId: entry.id });
    await eventually(() => devtools.evaluate(`location.href === ${JSON.stringify(url)} && document.querySelector('[data-testid="task-discussion"]')?.textContent.includes('页面冻结期间的原生更新')`), '实际后退并恢复查询');
    const after = await devtools.evaluate('({proof: globalThis.__kanbanBfCacheProof, draft: document.querySelector("textarea[name=comment-body]")?.value, focus: document.activeElement?.getAttribute("name"), buildId: document.querySelector("main")?.dataset.runtimeWebBuildId})');
    report.before = before;
    report.after = after;
    assert.equal(after.proof.documentId, before.proof.documentId, '必须恢复原 Document，不能重新加载');
    assert(after.proof.events.some(event => event.type === 'pageshow' && event.persisted && event.trusted), '必须观测真实 BFCache pageshow');
    assert(after.proof.events.some(event => event.type === 'pagehide' && event.persisted && event.trusted));
    assert.equal(after.draft, before.draft);
    assert.equal(after.focus, before.focus);
    assert(!(await cli('comment', 'list', task.id)).data.some(comment => comment.body === before.draft), '草稿不得被隐式发布');
    report.requests = rpcRequests(devtools.events).map(request => ({ url: request.url, method: request.method, headers: request.headers }));
    assert(report.requests.length > 1);
    assert(report.requests.every(request => request.method === 'POST' && request.url.endsWith('/WatchQueries') && Object.entries(request.headers).some(([key, value]) => key.toLowerCase() === 'content-type' && value === 'application/grpc-web+proto')));
    report.assertions.push('实际缓存恢复且 Document 身份一致', 'pagehide/pageshow 均为浏览器 trusted persisted 事件', '冻结期间原生写入在恢复后可见', '未保存草稿和输入焦点保持', '草稿未隐式写入', '恢复只使用 binary gRPC-Web 查询流', 'Host/runtime/Web artifact 身份一致');
    await writeFile(path.join(output, 'restored.png'), Buffer.from((await devtools.send('Page.captureScreenshot')).data, 'base64'));
    await click(devtools, 'document.querySelector("main h1")');
    assert.equal(await devtools.evaluate('document.activeElement?.getAttribute("name")'), null);
    await devtools.send('Page.navigate', { url: baseUrl + '/health' });
    await eventually(() => devtools.evaluate(`location.href === ${JSON.stringify(baseUrl + '/health')}`), '第二次离开应用');
    await devtools.send('Page.navigateToHistoryEntry', { entryId: entry.id });
    await eventually(() => devtools.evaluate(`location.href === ${JSON.stringify(url)} && globalThis.__kanbanBfCacheProof?.events.filter(event => event.type === 'pageshow' && event.persisted).length === 2`), '第二次真实缓存恢复');
    await devtools.evaluate('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
    report.afterExplicitBlur = await devtools.evaluate('({proof: globalThis.__kanbanBfCacheProof, focus: document.activeElement?.getAttribute("name"), draft: document.querySelector("textarea[name=comment-body]")?.value})');
    assert.equal(report.afterExplicitBlur.proof.documentId, before.proof.documentId);
    assert.equal(report.afterExplicitBlur.focus, null, '不能恢复用户主动移开的旧焦点');
    assert.equal(report.afterExplicitBlur.draft, before.draft);
    report.assertions.push('用户主动移开焦点后再次缓存恢复不会抢回旧输入框');
    report.passed = true;
  } catch (error) {
    report.error = error.stack;
    if (devtools) {
      report.notRestoredReasons = devtools.events.filter(event => event.method === 'Page.backForwardCacheNotUsed');
      report.failureRequests = rpcRequests(devtools.events);
      try { report.lastPage = await devtools.evaluate('({url: location.href, proof: globalThis.__kanbanBfCacheProof, text: document.body.innerText})'); } catch { /* 失败时保留已取得证据。 */ }
      try { await writeFile(path.join(output, 'failed.png'), Buffer.from((await devtools.send('Page.captureScreenshot')).data, 'base64')); } catch { /* 页面已退出时继续回收。 */ }
    }
  } finally {
    const cleanupErrors = [];
    if (devtools) {
      try { await devtools.send('Browser.close'); } catch { /* 已退出时继续进程回收。 */ }
      devtools.close();
    }
    // 每个进程独立回收；浏览器退出失败也必须继续停止隔离 Host 并写出失败证据。
    for (const [child, exited, signal, label] of [
      [browser, () => browserExit, 'SIGTERM', 'Chromium'],
      [host, () => hostExit, 'SIGINT', 'Host'],
    ]) {
      if (!child || exited()) continue;
      try {
        child.kill(signal);
        await eventually(exited, `${label} 正常退出`);
      } catch (error) {
        cleanupErrors.push(error.message);
        child.kill('SIGKILL');
        try { await eventually(exited, `${label} 强制回收`); }
        catch (error) { cleanupErrors.push(error.message); }
      }
    }
    const listenerClosed = await fetch(baseUrl + '/health', { signal: AbortSignal.timeout(500) }).then(() => false, () => true);
    report.cleanup = { hostExit, browserExit, hostGone: host ? !await alive(host.pid) : true, browserGone: browser ? !await alive(browser.pid) : true, listenerClosed, errors: cleanupErrors };
    if (cleanupErrors.length || hostExit?.code !== 0 || !report.cleanup.hostGone || !report.cleanup.browserGone || !listenerClosed) report.passed = false;
    report.finishedAt = new Date().toISOString();
    await writeFile(path.join(output, 'host.log'), Buffer.concat(hostLog));
    await writeFile(path.join(output, 'browser.log'), Buffer.concat(browserLog));
    await writeFile(path.join(output, 'result.json'), JSON.stringify(report, null, 2) + '\n');
  }
  assert(report.passed, report.error);
  assert.equal(report.cleanup.hostExit.code, 0);
  assert(report.cleanup.hostGone && report.cleanup.browserGone && report.cleanup.listenerClosed);
  process.stdout.write(JSON.stringify({ passed: true, assertions: report.assertions, result: path.join(output, 'result.json') }) + '\n');
}

const [binary, webDirectory, output, ...extra] = process.argv.slice(2);
if (!binary || !webDirectory || !output || extra.length) throw new Error('用法：bfcache-proof.mjs <候选 kanban binary> <Web dist> <新证据目录>');
await proof(path.resolve(binary), path.resolve(webDirectory), path.resolve(output));
