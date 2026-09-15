import { expect, test } from '@playwright/test';
import { installExplorerFixture } from './explorer-fixture';

for (const width of [1440, 1024, 390]) {
  for (const theme of ['light', 'dark']) {
    test(`纸本任务与详情 ${width}px ${theme}`, async ({ page }, info) => {
      const errors: string[] = [];
      page.on('pageerror', error => errors.push(error.message));
      await page.setViewportSize({ width, height: 900 });
      await page.addInitScript(mode => localStorage.setItem('kb:web:theme', mode), theme);
      await installExplorerFixture(page);
      await page.goto('/app/boards/default/list?task=t_ready');
      await expect(page.getByTestId('task-list')).toBeVisible();
      await expect(page.getByTestId('task-inspector')).toBeVisible();
      await expect(page.locator('html')).toHaveAttribute('data-resolved-theme', theme);
      await expect(page.getByRole('dialog')).toBeVisible();
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
      await page.evaluate(async () => { await Promise.all(document.getAnimations().filter(animation => animation.effect?.getTiming().iterations !== Infinity).map(animation => animation.finished.catch(() => undefined))); });
      await page.screenshot({ path: info.outputPath(`paper-${width}-${theme}.png`), fullPage: true });
      await page.keyboard.press('Escape');
      await expect(page.getByTestId('task-inspector')).toHaveCount(0);
      expect(errors).toEqual([]);
    });
  }
}

test('已删除页面的旧链接返回同项目列表，不请求专属接口', async ({ page }) => {
  const fixture = await installExplorerFixture(page);
  for (const retired of ['signals', 'ontology']) {
    await page.goto(`/app/boards/default/${retired}?signal=removed`);
    await expect(page).toHaveURL(/\/app\/boards\/default\/list\?notice=page-removed$/);
    await expect(page.getByTestId('task-list')).toBeVisible();
    await expect(page.getByText('该功能已移除，已返回当前项目任务列表。')).toBeVisible();
    await expect(page.getByTestId(`nav-${retired}`)).toHaveCount(0);
  }
  expect(fixture.apiRequests.filter(path => /signals|ontology/.test(path))).toEqual([]);
});
