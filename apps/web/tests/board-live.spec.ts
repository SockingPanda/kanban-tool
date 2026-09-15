import { expect, test } from '@playwright/test';
import { installBoardFixture } from './runtime-fixture';

test('首页进入任务列表，四列看板保留全部活动状态',async({page})=>{
  const fixture=await installBoardFixture(page);await page.goto('/app/');
  await expect(page).toHaveURL(/\/app\/boards\/default\/list$/);
  await expect(page.getByTestId('task-row')).toHaveCount(8);
  await page.getByRole('button',{name:'看板',exact:true}).click();
  await expect(page.locator('.board-column')).toHaveCount(4);
  await expect(page.getByTestId('board-task')).toHaveCount(8);
  expect(fixture.apiRequests.filter(path=>path.includes('/tasks/by-status'))).toEqual([]);
  const skip=page.getByRole('link',{name:'跳到主要内容'});await skip.focus();await skip.press('Enter');
  await expect(page.locator('#main-content')).toBeFocused();
});
test('无项目时展示空状态，不虚构任务或列',async({page})=>{
  const fixture=await installBoardFixture(page,{emptyBoards:true});await page.goto('/app/');
  await expect(page.locator('main .paper-banner')).toBeVisible();
  await expect(page.getByTestId('board-task')).toHaveCount(0);
  expect(fixture.apiRequests.some(path=>path.endsWith('/columns'))).toBe(false);
});
test('设置页只读取目录，不读取任务查询',async({page})=>{
  const fixture=await installBoardFixture(page);await page.goto('/app/settings');
  await expect(page.getByTestId('settings-page')).toBeVisible();
  expect(fixture.apiRequests.filter(path=>path.includes('/tasks'))).toEqual([]);
});
test('SSE 更新列表，离线保留快照并恢复连接',async({page,context})=>{
  const fixture=await installBoardFixture(page);await page.goto('/app/boards/default/list');
  await expect(page.getByTestId('task-list')).toContainText('Ready task');
  await fixture.waitForSseConnection(0);const initial=await fixture.getSseConnectionCount();
  fixture.setReadyTaskTitle('SSE 更新任务');await fixture.emitTaskUpdated();
  await expect(page.getByTestId('task-list')).toContainText('SSE 更新任务');
  await context.setOffline(true);await expect(page.getByTestId('task-sync-notice')).toBeVisible();await expect(page.getByTestId('task-list')).toContainText('SSE 更新任务');
  // 内存 SSE 不经过浏览器网络层，显式断开以模拟真实连接断线。
  await fixture.closeSse();
  await context.setOffline(false);await fixture.waitForSseConnection(initial);
  await expect(page.getByTestId('task-list')).toContainText('SSE 更新任务');
});
