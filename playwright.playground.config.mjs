import { defineConfig } from '@playwright/test';

const port = Number(process.env.WALZ_PLAYGROUND_TEST_PORT ?? 41740);
const root = process.env.WALZ_PLAYGROUND_TEST_ROOT ?? 'target/playground';

export default defineConfig({
  testDir: 'playground/tests',
  workers: 1,
  timeout: 30_000,
  expect: { timeout: 15_000 },
  use: { baseURL: `http://127.0.0.1:${port}`, browserName: 'chromium', headless: true },
  webServer: {
    command: 'node tools/serve-playground.mjs',
    env:{WALZ_PLAYGROUND_TEST_ROOT:root,WALZ_PLAYGROUND_TEST_PORT:String(port)},
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: false,
  },
});
