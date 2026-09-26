import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "tests",
  testMatch: "*.browser.ts",
  workers: 1,
  use: { headless: true, viewport: { width: 1440, height: 1000 } },
  timeout: 30000,
});
