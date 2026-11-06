import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: 'playground/tests',
  workers: 1,
  timeout: 30_000,
  expect: { timeout: 15_000 },
  use: { baseURL: 'http://127.0.0.1:41740', browserName: 'chromium', headless: true },
  webServer: {
    command: 'node tools/serve-playground.mjs',
    url: 'http://127.0.0.1:41740',
    reuseExistingServer: false,
  },
});
