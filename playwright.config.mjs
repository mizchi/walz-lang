import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "runtime/e2e",
  workers: 1,
  timeout: 60000,
  use: {
    baseURL: "http://127.0.0.1:41739",
    browserName: "chromium",
    headless: true,
  },
  webServer: {
    command: "node runtime/serve.mjs",
    url: "http://127.0.0.1:41739",
    timeout: 300000,
    reuseExistingServer: false,
  },
});
