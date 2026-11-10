import {fileURLToPath} from 'node:url';
import {defineConfig} from 'vite';
import wlt from '@mizchi/wlt-runtime/vite';

export default defineConfig({
  root:fileURLToPath(new URL('.', import.meta.url)),
  plugins:[wlt({dts:true})],
  build:{target:'esnext', outDir:'../../target/vite-example'},
});
