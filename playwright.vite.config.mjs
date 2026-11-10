import {defineConfig} from '@playwright/test';

export default defineConfig({
  testDir:'vite/e2e', workers:1, timeout:30000, expect:{timeout:15000},
  use:{browserName:'chromium', headless:true},
  projects:[
    {name:'dev',use:{baseURL:'http://127.0.0.1:41742/wlt-vite/'}},
    {name:'preview',use:{baseURL:'http://127.0.0.1:41743/wlt-vite/'}},
  ],
  webServer:[
    {command:'node vite/test-server.mjs dev 41742',url:'http://127.0.0.1:41742/wlt-vite/',reuseExistingServer:false,timeout:60000},
    {command:'node vite/test-server.mjs preview 41743',url:'http://127.0.0.1:41743/wlt-vite/',reuseExistingServer:false,timeout:60000},
  ],
});
