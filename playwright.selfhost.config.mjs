import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "runtime/e2e",
  testMatch: "selfhost.spec.mjs",
  workers: 1,
  timeout: 60000,
  use: {
    baseURL: "http://127.0.0.1:41741",
    browserName: "chromium",
    headless: true,
  },
  webServer: {
    command: "node runtime/selfhost/serve.mjs",
    url: "http://127.0.0.1:41741",
    timeout: 180000,
    reuseExistingServer: false,
  },
});
