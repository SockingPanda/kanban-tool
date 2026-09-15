import { mkdir, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { expect, test, type Page, type APIRequestContext } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

const baseURL = process.env.KANBAN_RELEASE_BASE_URL!;
const evidence = process.env.KANBAN_RELEASE_EVIDENCE_DIR ?? '../../output/atlas-paper/host';
const runId = process.env.KANBAN_RELEASE_RUN_ID!;
let taskId = '', title = '', parentId = '', buildId = '';
const errors: string[] = [], completed: string[] = [];
async function choose(page: Page, label: string, value: string) {
  await page.getByRole('combobox', { name: label, exact: true }).click();
  await page.getByRole('option', { name: value, exact: true }).click();
}
async function read(request: APIRequestContext, suffix = '') {
  const response = await request.get(`${baseURL}/api/v1/tasks/${taskId}${suffix}`);
  expect(response.ok(), await response.text()).toBe(true);
  return (await response.json()).data;
}
async function detail(page: Page) {
  await page.goto(`/app/boards/default/list?task=${taskId}`);
  await expect(page.getByTestId('task-inspector')).toBeVisible();
}
async function apiCreate(request: APIRequestContext, taskTitle: string) {
  const response = await request.post(`${baseURL}/api/v1/boards/default/tasks`, { data: { title: taskTitle, description: '验收所需的隔离任务', actor: 'v4-proof' } });
  expect(response.ok(), await response.text()).toBe(true);
  return (await response.json()).data;
}

test.describe.configure({ mode: 'serial' });
test.beforeEach(async ({ page }) => {
  page.on('pageerror', error => errors.push(error.message));
  await page.addInitScript(() => {
    (window as unknown as { cspErrors: string[] }).cspErrors = [];
    document.addEventListener('securitypolicyviolation', event => (window as unknown as { cspErrors: string[] }).cspErrors.push(`${event.violatedDirective}:${event.blockedURI}`));
  });
});
test.afterAll(async ({ request }, info) => {
  void request;
  await mkdir(evidence, { recursive: true });
  await writeFile(`${evidence}/paper-${info.project.name}-${runId}.json`, JSON.stringify({ runId, browser: info.project.name, baseURL, buildId, taskId, parentId, completed, errors }, null, 2));
});

test('真实产物身份、创建任务、标题和说明保存', async ({ page, request }, info) => {
  const runtime = await (await request.get(`${baseURL}/app/runtime.json`)).json();
  const manifest = await (await request.get(`${baseURL}/app/manifest.json`)).json();
  const health = await (await request.get(`${baseURL}/health`)).json();
  expect(runtime.serverVersion).toBe('3.1.0');
  expect(health.data.version).toBe(runtime.serverVersion);
  expect(runtime.webBuildId).toBe(manifest.buildId);
  buildId = runtime.webBuildId;
  const response = await page.goto('/app/boards/default/list');
  expect(response?.headers()['content-security-policy']).toContain("style-src 'self'");
  await expect(page.locator('main')).toHaveAttribute('data-runtime-web-build-id', buildId);
  await expect(page.getByTestId('task-create')).toBeEnabled();
  await page.getByTestId('task-create').click();
  title = `纸本验收 ${info.project.name} ${Date.now()}`;
  const form = page.getByTestId('task-mutation-dialog');
  await form.getByTestId('task-title-input').fill(title);
  await form.getByTestId('task-description-input').fill('确认真实 Host 保存任务');
  await form.getByText('执行计划', { exact: true }).click();
  await form.getByTestId('first-required-step-input').fill('确认真实运行结果');
  await form.getByRole('button', { name: '创建任务', exact: true }).click();
  await expect(form).toBeHidden();
  await expect(page).toHaveURL(/task=t_/);
  taskId = new URL(page.url()).searchParams.get('task')!;
  await expect(page.getByTestId('task-inspector')).toBeVisible();
  expect((await read(request)).title).toBe(title);
  const field = page.getByRole('textbox', { name: '任务标题', exact: true });
  title += ' 已编辑';
  await field.fill(title); await field.press('Tab');
  await expect.poll(async () => (await read(request)).title).toBe(title);
  const description = page.getByRole('textbox', { name: '编辑任务说明' });
  await description.fill('长描述\n'.repeat(100)); await description.press('Tab');
  await expect.poll(async () => (await read(request)).description).toBe('长描述\n'.repeat(100).trim());
  await page.reload();
  await expect(field).toHaveValue(title);
  expect(await page.evaluate(() => (window as unknown as { cspErrors: string[] }).cspErrors)).toEqual([]);
  completed.push('identity', 'create', 'edit', 'refresh-persistence');
});

test('步骤编辑、完成、重开、跳过及删除', async ({ page, request }) => {
  await detail(page);
  const steps = page.getByTestId('task-inspector-steps');
  const checkbox = steps.getByRole('checkbox', { name: '确认真实运行结果', exact: true });
  await checkbox.click();
  await expect(checkbox).toBeChecked();
  await expect.poll(async () => (await read(request, '/steps')).steps[0].status).toBe('done');
  await checkbox.click();
  await expect(checkbox).not.toBeChecked();
  await expect.poll(async () => (await read(request, '/steps')).steps[0].status).toBe('todo');
  await steps.getByRole('button', { name: '确认真实运行结果', exact: true }).click();
  const editor = page.getByRole('dialog', { name: '执行步骤', exact: true });
  await editor.getByRole('textbox', { name: '步骤说明' }).fill('从真实服务回读验证');
  await editor.getByRole('button', { name: '保存', exact: true }).click();
  await expect(editor).toBeHidden();
  expect((await read(request, '/steps')).steps[0].body).toBe('从真实服务回读验证');
  await steps.getByRole('textbox', { name: '新的执行步骤' }).fill('临时步骤');
  await steps.getByRole('button', { name: '添加步骤', exact: true }).click();
  await steps.getByRole('button', { name: '临时步骤', exact: true }).click();
  await editor.getByRole('textbox', { name: '跳过原因' }).fill('测试完成，无需重复执行');
  await editor.getByRole('button', { name: '跳过步骤' }).click();
  await expect(editor).toBeHidden();
  await steps.getByRole('button', { name: '删除步骤 临时步骤' }).click();
  await expect.poll(async () => (await read(request, '/steps')).steps.length).toBe(1);
  completed.push('steps');
});

test('依赖添加、环拒绝和删除，讨论及普通标签、附件', async ({ page, request }, info) => {
  const parent = await apiCreate(request, `前置 ${info.project.name} ${Date.now()}`); parentId = parent.id;
  await detail(page);
  await page.getByRole('combobox', { name: '添加依赖任务', exact: true }).click();
  await page.getByRole('combobox', { name: '搜索添加依赖任务' }).fill(parent.title);
  await page.getByRole('option', { name: `${parent.ref} · ${parent.title}`, exact: true }).click();
  await expect.poll(async () => (await read(request, '/dependencies')).parents.map((task: { id: string }) => task.id)).toContain(parentId);
  const cycle = await request.post(`${baseURL}/api/v1/tasks/${parentId}/dependencies`, { data: { depends_on: taskId, actor: 'v4-proof' } });
  expect(cycle.ok()).toBe(false);
  await page.getByRole('button', { name: `解除依赖 ${parent.ref}` }).click();
  await expect.poll(async () => (await read(request, '/dependencies')).parents.length).toBe(0);
  await page.getByRole('button', { name: /^讨论/ }).click();
  await page.getByRole('textbox', { name: '评论内容' }).fill(`已验证 ${info.project.name}`);
  await page.getByRole('button', { name: '发布评论' }).click();
  await expect(page.getByRole('textbox', { name: '评论内容' })).toHaveValue('');
  expect((await read(request, '/comments')).some((comment: { body: string }) => comment.body === `已验证 ${info.project.name}`)).toBe(true);
  await page.getByRole('button', { name: '详情', exact: true }).click();
  await page.getByText('标签与附件', { exact: true }).first().click();
  await page.getByRole('textbox', { name: '标签名称' }).fill('Stage09 release');
  await page.getByTestId('label-add').click();
  await expect(page.getByTestId('inspector-labels')).toContainText('Stage09 release');
  await page.getByTestId('label-suggestion-request').click();
  await expect(page.getByTestId('label-suggestions')).toBeVisible();
  const content = Buffer.from('真实附件\n');
  await page.getByTestId('attachment-file').setInputFiles({ name: 'paper.txt', mimeType: 'text/plain', buffer: content });
  await page.getByTestId('attachment-upload').click();
  await expect(page.getByTestId('attachment-row')).toContainText('paper.txt');
  const attachment = (await read(request, '/attachments'))[0];
  expect(attachment.sha256).toBe(createHash('sha256').update(content).digest('hex'));
  const [download] = await Promise.all([page.waitForEvent('download'), page.getByTestId('attachment-download').click()]);
  expect(download.suggestedFilename()).toBe('paper.txt');
  await page.getByTestId('attachment-delete').click();
  await expect(page.getByTestId('attachment-row')).toHaveCount(0);
  await page.getByTestId('label-remove').click();
  await expect.poll(async () => (await read(request, '/labels')).length).toBe(0);
  completed.push('dependencies', 'cycle-rejected', 'comments', 'labels', 'label-suggestions', 'attachments');
});

test('失败保留草稿，合法状态动作、非法目标、运行记录', async ({ page, request }) => {
  await detail(page);
  let failed = false;
  await page.route(`**/api/v1/tasks/${taskId}`, async route => {
    if (route.request().method() === 'PATCH' && !failed) { failed = true; await route.abort('failed'); }
    else await route.continue();
  });
  const field = page.getByRole('textbox', { name: '任务标题', exact: true });
  const draft = `${title} 保留草稿`;
  await field.fill(draft); await field.press('Tab');
  await expect(page.getByTestId('inspector-mutation-error').first()).toBeVisible();
  await expect(field).toHaveValue(draft);
  expect((await read(request)).title).toBe(title);
  await page.getByTestId('inspector-mutation-error').first().getByRole('button', { name: '重试' }).click();
  await expect.poll(async () => (await read(request)).title).toBe(draft); title = draft;
  await page.getByRole('combobox', { name: '更改任务状态' }).click();
  await expect(page.getByRole('option', { name: '已完成', exact: true })).toHaveAttribute('aria-disabled', 'true');
  await page.keyboard.press('Escape');
  await choose(page, '更改任务状态', '已就绪');
  await expect.poll(async () => (await read(request)).status).toBe('ready');
  await choose(page, '更改任务状态', '进行中');
  await expect.poll(async () => (await read(request)).status).toBe('running');
  await page.getByText('运行记录与任务操作', { exact: true }).click();
  const [heartbeat] = await Promise.all([
    page.waitForResponse(response => response.url().endsWith('/transitions/heartbeat') && response.request().method() === 'POST'),
    page.getByRole('button', { name: '发送心跳', exact: true }).click(),
  ]);
  expect(heartbeat.ok()).toBe(true);
  await page.getByText('运行记录与任务操作', { exact: true }).click();
  const completeStep = page.getByTestId('task-inspector-steps').getByRole('checkbox', { name: '确认真实运行结果', exact: true });
  await completeStep.click();
  await expect(completeStep).toBeChecked();
  await choose(page, '更改任务状态', '待验收');
  await expect.poll(async () => (await read(request)).status).toBe('review');
  await choose(page, '更改任务状态', '已完成');
  await expect.poll(async () => (await read(request)).status).toBe('done');
  await page.getByText('运行记录与任务操作', { exact: true }).click();
  await page.getByRole('button', { name: '查看运行记录', exact: true }).click();
  await expect(page.getByTestId('runs-ready')).toBeVisible();
  expect((await read(request, '/runs')).length).toBeGreaterThan(0);
  completed.push('draft-on-failure', 'transition', 'invalid-transition-disabled', 'heartbeat', 'runs');
});

for (const width of [1440, 1024, 390]) for (const theme of ['light', 'dark']) {
  test(`真实页面 ${width}px ${theme} 与键盘焦点`, async ({ page }, info) => {
    await page.setViewportSize({ width, height: 900 });
    await page.addInitScript(value => localStorage.setItem('kb:web:theme', value), theme);
    await detail(page);
    await expect(page.locator('html')).toHaveAttribute('data-resolved-theme', theme);
    await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
    await page.evaluate(async () => { await Promise.all(document.getAnimations().filter(animation => animation.effect?.getTiming().iterations !== Infinity).map(animation => animation.finished.catch(() => undefined))); });
    await page.screenshot({ path: `${evidence}/paper-${info.project.name}-${width}-${theme}-${runId}.png`, fullPage: true });
    if (width === 1440 && theme === 'light') {
      const result = await new AxeBuilder({ page }).withTags(['wcag2a','wcag2aa','wcag21aa']).analyze();
      expect(result.violations.map(item => ({ id:item.id, nodes:item.nodes.map(node=>node.target) }))).toEqual([]);
    }
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('task-inspector')).toHaveCount(0);
    if (width === 1440) {
      const row = page.getByTestId('task-row').filter({ hasText:title }).getByRole('button').first();
      await row.focus(); await row.press('Enter');
      await expect(page.getByTestId('task-inspector')).toBeVisible();
      await page.keyboard.press('Escape'); await expect(row).toBeFocused();
    }
    expect(errors).toEqual([]);
    expect(await page.evaluate(() => (window as unknown as { cspErrors: string[] }).cspErrors)).toEqual([]);
    completed.push(`visual-${width}-${theme}`);
  });
}
