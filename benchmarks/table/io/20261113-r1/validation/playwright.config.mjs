import {defineConfig} from '@playwright/test';
import {fileURLToPath} from 'node:url';
export default defineConfig({testDir:'../../runtime/e2e',testMatch:'table.spec.mjs',workers:1,timeout:60000,
  use:{baseURL:'http://127.0.0.1:41739',browserName:'chromium',headless:true},
  webServer:{command:'node target/table-io/browser-server.mjs',cwd:fileURLToPath(new URL('../../',import.meta.url)),url:'http://127.0.0.1:41739',timeout:180000,reuseExistingServer:false}});
