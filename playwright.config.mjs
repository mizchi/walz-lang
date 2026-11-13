import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "runtime/e2e",
  // These suites have their own servers and configurations.
  testIgnore: ["**/composition.spec.mjs", "**/selfhost.spec.mjs"],
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
    timeout: 600000,
    reuseExistingServer: false,
  },
});
