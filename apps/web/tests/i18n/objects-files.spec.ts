import { expect, test, type Page } from '@playwright/test';
import { readFile } from 'node:fs/promises';

const workspace = (page: Page) => page.getByTestId('object-workspace');
async function createModule(page: Page, title: string) {
  await workspace(page).getByRole('button', { name: '新建对象', exact: true }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel('标题', { exact: true }).fill(title);
  await dialog.getByLabel('正文', { exact: true }).fill('保留原文 <literal> {{name}}');
  await dialog.getByRole('button', { name: '保存', exact: true }).click();
  await expect(dialog).not.toBeVisible();
  await expect(workspace(page).locator('main').getByLabel('标题', { exact: true })).toHaveValue(title);
  return (await workspace(page).locator('main header code').textContent())!;
}

test('真实对象与附件支持实时更新、CAS 草稿冲突、下载和多对象解除引用', async ({ page, context }, info) => {
  test.setTimeout(120000);
  await page.addInitScript(() => localStorage.setItem('kb:web:locale', 'zh'));
  await page.goto('boards/default/list?objects=module');
  await expect(workspace(page).getByText('实时更新已连接', { exact: true })).toBeVisible();
  const title = `整合模块 ${Date.now()}`;
  const owner = await createModule(page, title);
  const second = await context.newPage();
  await second.goto('boards/default/list?objects=module');
  await workspace(second).locator('aside button').filter({ hasText: title }).click();
  const draft = workspace(page).locator('main').getByLabel('标题', { exact: true });
  await draft.fill(`${title} 本地草稿`);
  await workspace(second).locator('main').getByLabel('标题', { exact: true }).fill(`${title} 他处更新`);
  await workspace(second).getByRole('button', { name: '保存标题', exact: true }).click();
  await expect(workspace(page).getByText('对象已在其他入口更新，当前草稿仍保留。保存时会检查原版本。', { exact: true })).toBeVisible();
  await workspace(page).getByRole('button', { name: '保存标题', exact: true }).click();
  await expect(workspace(page).locator('[role=alert]').first()).toBeVisible();
  await expect(draft).toHaveValue(`${title} 本地草稿`);
  await workspace(page).getByRole('button', { name: '丢弃当前草稿', exact: true }).click();
  await expect(draft).toHaveValue(`${title} 他处更新`);
  await second.close();

  const manager = page.getByTestId('attachment-manager');
  const bytes = Buffer.from('附件真实字节\nhex:保持普通文本');
  await manager.locator('input[type=file]').setInputFiles([
    { name: '整合证据.txt', mimeType: 'text/plain', buffer: bytes },
    { name: 'empty.txt', mimeType: 'text/plain', buffer: Buffer.alloc(0) },
  ]);
  await expect(manager.locator('ul li')).toHaveCount(2);
  const attachment = manager.locator('ul li').filter({ hasText: '整合证据.txt' });
  await attachment.getByText('哈希与标识', { exact: true }).click();
  const fileId = (await attachment.locator('code').innerText()).split('\n')[0];
  const downloadPromise = page.waitForEvent('download');
  await attachment.getByRole('button', { name: '下载', exact: true }).click();
  const download = await downloadPromise;
  expect(await readFile((await download.path())!)).toEqual(bytes);
  await download.saveAs(info.outputPath(download.suggestedFilename()));

  const otherTitle = `${title} 共享对象`;
  const other = await createModule(page, otherTitle);
  await workspace(page).getByRole('button', { name: 'file.attachment', exact: true }).click();
  await workspace(page).getByLabel('关联对象 ID', { exact: true }).fill(fileId);
  await workspace(page).getByRole('button', { name: '添加关系', exact: true }).click();
  await expect(manager.locator('ul li')).toHaveCount(1);
  await manager.getByRole('button', { name: '解除引用', exact: true }).click();
  await manager.getByRole('button', { name: '确认解除引用', exact: true }).click();
  await expect(manager.locator('ul li')).toHaveCount(0);
  await workspace(page).locator('aside button').filter({ hasText: `${title} 他处更新` }).click();
  await expect(manager.locator('ul li')).toHaveCount(2);
  await page.reload();
  await workspace(page).locator('aside button').filter({ hasText: `${title} 他处更新` }).click();
  await expect(manager.locator('ul li')).toHaveCount(2);
  await info.attach('object-file-identities', { body: JSON.stringify({ owner, other, fileId }), contentType: 'application/json' });
  await page.screenshot({ path: info.outputPath('objects-and-files.png'), fullPage: true });
  await manager.scrollIntoViewIfNeeded();
  await page.screenshot({ path: info.outputPath('shared-attachments.png') });
});

test('迭代校验时间并通过键盘完成开始与关闭', async ({ page }, info) => {
  await page.addInitScript(() => localStorage.setItem('kb:web:locale', 'zh'));
  await page.goto('boards/default/list?objects=cycle');
  await expect(workspace(page).getByText('实时更新已连接', { exact: true })).toBeVisible();
  await workspace(page).getByRole('button', { name: '新建对象', exact: true }).click();
  const dialog = page.getByRole('dialog');
  const title = `迭代验收 ${Date.now()}`;
  await dialog.getByLabel('标题', { exact: true }).fill(title);
  await dialog.getByLabel('开始时间').fill('2026-09-16T09:00');
  await dialog.getByLabel('结束时间').fill('2026-09-15T09:00');
  await dialog.getByRole('button', { name: '保存', exact: true }).press('Enter');
  await expect(dialog.getByRole('alert')).toHaveText('开始、结束时间必须有效，且结束时间晚于开始时间。');
  await dialog.getByLabel('结束时间').fill('2026-09-30T09:00');
  await dialog.getByRole('button', { name: '保存', exact: true }).press('Enter');
  await expect(dialog).not.toBeVisible();
  await expect(workspace(page).locator('main').getByLabel('标题', { exact: true })).toHaveValue(title);
  await workspace(page).getByRole('button', { name: '开始迭代', exact: true }).press('Enter');
  await dialog.getByRole('button', { name: '确认操作', exact: true }).press('Enter');
  await expect(dialog).not.toBeVisible();
  await workspace(page).getByRole('button', { name: '关闭迭代', exact: true }).press('Enter');
  await dialog.getByRole('button', { name: '确认操作', exact: true }).press('Enter');
  await expect(dialog).not.toBeVisible();
  await expect(workspace(page).locator('main').getByLabel('标题', { exact: true })).toBeDisabled();
  await page.reload();
  await workspace(page).locator('aside button').filter({ hasText: title }).click();
  await expect(workspace(page).locator('main').getByLabel('标题', { exact: true })).toBeDisabled();
  await page.screenshot({ path: info.outputPath('closed-cycle.png'), fullPage: true });
});

test('任务附件经统一文件接口传输 5 MiB，并在刷新后保持解除引用结果', async ({ page }, info) => {
  test.setTimeout(120000);
  const methods: string[] = [];
  page.on('request', request => {
    const path = new URL(request.url()).pathname;
    if (path.startsWith('/kanban.extensions.v1.FileService/')) methods.push(path.split('/').at(-1)!);
  });
  await page.addInitScript(() => localStorage.setItem('kb:web:locale', 'zh'));
  await page.goto('boards/default/list');
  const task = page.getByRole('table', { name: '任务列表' }).getByRole('button', { name: 'i18n 原文 <tag> $t(common:save)' });
  await expect(task).toBeVisible();
  await task.click();
  await expect(page.getByTestId('task-inspector')).toBeVisible();
  const manager = page.getByTestId('attachment-manager');
  const bytes = Buffer.alloc(5 * 1024 * 1024, 0xab);
  await manager.getByTestId('attachment-file').setInputFiles({ name: 'task-5MiB.bin', mimeType: 'application/octet-stream', buffer: bytes });
  await expect(manager.getByTestId('attachment-row')).toHaveCount(1);
  const downloadEvent = page.waitForEvent('download');
  await manager.getByTestId('attachment-download').click();
  const download = await downloadEvent;
  expect(await readFile((await download.path())!)).toEqual(bytes);
  expect(methods).toContain('BeginFileUpload');
  expect(methods).toContain('FinishFileUpload');
  expect(methods).toContain('DownloadFile');
  await manager.getByTestId('attachment-delete').click();
  await manager.getByRole('button', { name: '确认解除引用', exact: true }).click();
  await expect(manager.getByTestId('attachment-row')).toHaveCount(0);
  await page.reload();
  await expect(manager.getByTestId('attachment-row')).toHaveCount(0);
  await expect(manager.getByText('暂无附件', { exact: true })).toBeVisible();
  await info.attach('task-file-methods', { body: JSON.stringify(methods), contentType: 'application/json' });
});
