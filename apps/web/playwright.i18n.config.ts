import { defineConfig } from "@playwright/test";
const baseURL = process.env.KANBAN_I18N_BASE_URL;
if (!baseURL) throw new Error("KANBAN_I18N_BASE_URL 必须指向已启动的独立测试 Host，并包含 /app/。");
export default defineConfig({
  testDir: "./tests/i18n",
  outputDir: "../../output/playwright/i18n",
  workers: 1,
  forbidOnly: Boolean(process.env.CI),
  retries: 0,
  use: { baseURL, browserName: "chromium", viewport: { width: 1280, height: 800 }, trace: "retain-on-failure" },
  reporter: [["list"], ["html", { outputFolder: "../../output/playwright/i18n-report", open: "never" }]],
});
