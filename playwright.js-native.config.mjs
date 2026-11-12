import {defineConfig} from '@playwright/test';
export default defineConfig({testDir:'runtime/js-native-e2e',workers:1,timeout:60000,use:{baseURL:'http://127.0.0.1:41741',browserName:'chromium',headless:true},webServer:{command:'node runtime/js-native-e2e/serve.mjs',url:'http://127.0.0.1:41741',timeout:180000,reuseExistingServer:false}});
