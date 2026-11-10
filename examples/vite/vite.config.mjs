import {fileURLToPath} from 'node:url';
import {defineConfig} from 'vite';
import walz from '@mizchi/walz-runtime/vite';

export default defineConfig({
  root:fileURLToPath(new URL('.', import.meta.url)),
  plugins:[walz({dts:true})],
  build:{target:'esnext', outDir:'../../target/vite-example'},
});
