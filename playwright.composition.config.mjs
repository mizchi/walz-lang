import {defineConfig} from '@playwright/test';
export default defineConfig({
  testDir:'runtime/e2e',testMatch:['composition.spec.mjs','wasm-use.spec.mjs'],workers:1,timeout:60000,
  use:{baseURL:'http://127.0.0.1:41742',browserName:'chromium',headless:true},
  webServer:{command:'node runtime/composition/serve.mjs',url:'http://127.0.0.1:41742',timeout:180000,reuseExistingServer:false},
});
