import { defineConfig } from '@playwright/test';

const port = Number(process.env.WLT_PLAYGROUND_TEST_PORT ?? 41740);

export default defineConfig({
  testDir: 'playground/tests',
  workers: 1,
  timeout: 30_000,
  expect: { timeout: 15_000 },
  use: { baseURL: `http://127.0.0.1:${port}`, browserName: 'chromium', headless: true },
  webServer: {
    command: `node tools/serve-playground.mjs target/playground ${port}`,
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: false,
  },
});
