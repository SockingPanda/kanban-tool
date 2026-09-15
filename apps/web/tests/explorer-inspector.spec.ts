import { expect, test } from '@playwright/test';
import { installExplorerFixture } from './explorer-fixture';

test('列表、看板、依赖图、动态都能打开详情并恢复焦点',async({page})=>{
  await installExplorerFixture(page);
  for(const view of ['list','board','map','events']) {
    await page.goto(`/app/boards/default/${view}`);
    const opener=view==='board'?page.getByTestId('board-task').filter({hasText:'Ready task'}):view==='map'?page.getByRole('button',{name:/检查任务 default#1 Ready task/}):view==='events'?page.getByRole('button',{name:'查看任务 t_ready'}):page.getByTestId('task-row').filter({hasText:'Ready task'}).getByRole('button');
    await opener.focus();await opener.press('Enter');
    await expect(page.getByTestId('task-inspector')).toBeVisible();
    await page.keyboard.press('Escape');await expect(page.getByTestId('task-inspector')).toHaveCount(0);await expect(opener).toBeFocused();
  }
});
test('刷新恢复筛选、搜索、选中任务和依赖图缩放',async({page})=>{
  await installExplorerFixture(page);
  await page.goto('/app/boards/default/list?status=ready&q=Ready&task=t_ready');
  await expect(page.getByTestId('task-inspector')).toBeVisible();await page.reload();
  await expect(page.getByTestId('list-search')).toHaveValue('Ready');await expect(page).toHaveURL(/status=ready&q=Ready&task=t_ready/);
  await page.goto('/app/boards/default/map?filter=ready&zoom=1.2&task=t_ready');await expect(page.getByTestId('task-map-zoom')).toHaveText('120%');
  await page.reload();await expect(page.getByTestId('task-map-zoom')).toHaveText('120%');
});
test('Events 复用一个持久 SSE 连接，Run 深链接读取真实形状日志',async({page})=>{
  const fixture=await installExplorerFixture(page);await page.goto('/app/boards/default/events');await expect(page.getByTestId('event-row')).toHaveCount(1);
  await fixture.waitForSseConnection(0);const connections=await fixture.getSseConnectionCount();
  await fixture.emitTaskUpdated();await expect(page.getByTestId('event-row')).toHaveCount(2);expect(await fixture.getSseConnectionCount()).toBe(connections);
  await page.goto('/app/boards/default/runs?task=t_ready');await expect(page.getByTestId('runs-log')).toContainText('playwright fixture log');await expect(page).toHaveURL(/task=t_ready/);
});
test('读取失败和空数据具有可操作边界',async({page})=>{
  await installExplorerFixture(page,{failList:true});await page.goto('/app/boards/default/list');await expect(page.getByTestId('task-list-error')).toBeVisible();await expect(page.getByTestId('task-list-error').getByRole('button',{name:'重试'})).toBeEnabled();
  const empty=await page.context().newPage();await installExplorerFixture(empty,{emptyList:true});await empty.goto('/app/boards/default/list');await expect(empty.getByTestId('task-row')).toHaveCount(0);await expect(empty.getByTestId('task-create')).toBeEnabled();await empty.close();
});
test('键盘快捷键、搜索和事件类型筛选',async({page})=>{
  await installExplorerFixture(page);await page.goto('/app/boards/default/list');await expect(page.getByTestId('task-list')).toBeVisible();
  await page.keyboard.press('Control+k');await expect(page.getByTestId('list-search')).toBeFocused();
  await page.getByTestId('task-create').focus();await page.keyboard.press('c');await expect(page.getByTestId('task-mutation-dialog')).toBeVisible();await page.keyboard.press('Escape');
  await page.goto('/app/boards/default/events');await page.getByText('事件类型筛选',{exact:true}).first().click();
  const input=page.getByRole('searchbox',{name:'事件类型筛选'});await input.fill('task.updated');await input.press('Enter');await expect(page).toHaveURL(/kind=task.updated/);await expect(page.getByTestId('events-filter-empty')).toBeVisible();
});
