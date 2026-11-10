import {cp, mkdir, rm} from 'node:fs/promises';
import {resolve, join} from 'node:path';
import {build, createServer, preview} from 'vite';
import walz from './index.mjs';

const mode = process.argv[2], port = Number(process.argv[3]);
if (!['dev', 'preview'].includes(mode) || !port) throw new Error('usage: test-server.mjs dev|preview PORT');
const root = resolve('target/vite-e2e', mode);
await rm(root, {recursive:true, force:true}); await mkdir(root, {recursive:true});
for (const file of ['index.html','main.ts','program.walz','helper.walz']) await cp(join('examples/vite',file),join(root,file));
const options = {root, configFile:false, base:'/walz-vite/', cacheDir:resolve('target/vite-e2e/cache',mode),
  plugins:[walz({compiler:resolve('target/debug/walz'), dts:true})],
  build:{target:'esnext', outDir:'dist'}, server:{host:'127.0.0.1',port,strictPort:true},
  preview:{host:'127.0.0.1',port,strictPort:true}};
let server;
if (mode === 'dev') { server = await createServer(options); await server.listen(); }
else { await build(options); server = await preview({...options,plugins:[]}); }
async function close() { await server.close(); process.exit(0); }
process.on('SIGTERM',close); process.on('SIGINT',close);
