import { expect, test } from '@playwright/test';

for (const viewport of [{ width: 1280, height: 800 }, { width: 390, height: 844 }]) {
  test(`真实设置保存语言和有效 actor，保留非法草稿，${viewport.width}px`, async ({ page }, info) => {
    await page.setViewportSize(viewport);
    await page.addInitScript(() => { if (!localStorage.getItem('kb:web:locale')) localStorage.setItem('kb:web:locale', 'zh'); });
    const errors: string[] = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.goto('settings');
    await expect(page.getByTestId('settings-page')).toBeVisible();
    const runtime = await (await page.request.get('runtime.json')).json();
    const manifest = await (await page.request.get('manifest.json')).json();
    expect(runtime.webBuildId).toBe(manifest.buildId);
    if (process.env.KANBAN_I18N_EXPECTED_BUILD_ID) expect(runtime.webBuildId).toBe(process.env.KANBAN_I18N_EXPECTED_BUILD_ID);
    await info.attach('served-identity', { body: JSON.stringify({ runtime, manifest }), contentType: 'application/json' });
    await page.getByRole('tab', { name: '操作身份', exact: true }).click();
    const actor = page.getByTestId('identity-actor');
    await actor.fill(`语言验收 ${viewport.width}`);
    await actor.press('Tab');
    await expect.poll(() => page.evaluate(() => localStorage.getItem('kb:web:actor'))).toBe(`语言验收 ${viewport.width}`);
    await actor.fill('   '); await actor.press('Tab');
    await expect(actor).toHaveAttribute('aria-invalid', 'true');
    await page.getByRole('tab', { name: '外观', exact: true }).click();
    const url = page.url(), locale = page.getByTestId('settings-locale');
    await locale.focus(); await locale.press('Enter');
    await page.getByRole('option', { name: 'English', exact: true }).click();
    await expect(locale).toBeFocused();
    await expect(page.locator('html')).toHaveAttribute('lang', 'en');
    await expect(page.getByRole('heading', { name: 'Settings', exact: true })).toBeVisible();
    expect(page.url()).toBe(url);
    expect(await page.evaluate(() => localStorage.getItem('kb:web:actor'))).toBe(`语言验收 ${viewport.width}`);
    await page.reload();
    await expect(page.locator('html')).toHaveAttribute('lang', 'en');
    await page.getByRole('tab', { name: 'Identity', exact: true }).click();
    await expect(actor).toHaveValue(`语言验收 ${viewport.width}`);
    await page.getByRole('tab', { name: 'Appearance', exact: true }).click();
    await locale.click(); await page.getByRole('option', { name: '简体中文', exact: true }).click();
    await expect(page.locator('html')).toHaveAttribute('lang', 'zh-CN');
    await page.reload();
    await expect(page.getByRole('heading', { name: '设置', exact: true })).toBeVisible();
    await page.goto('boards/default/list');
    await expect(page.getByText('i18n 原文 <tag> $t(common:save)', { exact: true }).first()).toBeVisible();
    expect(errors).toEqual([]);
    await page.screenshot({ path: info.outputPath('locale-and-original-text.png'), fullPage: true });
  });
}
