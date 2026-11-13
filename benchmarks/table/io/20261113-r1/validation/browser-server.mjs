// Table-only validation server: fresh portable builds, verified raw artifact.
import {createServer} from 'node:http';
import {readFile,mkdir,writeFile,copyFile} from 'node:fs/promises';
import {resolve,join,extname} from 'node:path';
import {build as buildCore} from '../../runtime/build-core.mjs';
import {build as buildComponent} from '../../runtime/build.mjs';
import {bundle} from '../../runtime/flatbuffers-size.mjs';
const root=resolve('target/table-io/browser-v2'); await mkdir(join(root,'table'),{recursive:true});
await copyFile('target/table-io/primary/core.wasm',join(root,'table/core.wasm'));
const result=await bundle(`export {instantiate,op,desc} from ${JSON.stringify(resolve('runtime/table/index.mjs'))};`,'table',join(root,'table'));
await writeFile(join(root,'table/entry.json'),JSON.stringify({entry:result.entry}));
for(const [backend,build] of [['core',buildCore],['component',buildComponent]])await build('examples/table/portable.walz',join(root,backend,'table'));
createServer(async(req,res)=>{
  try {
    const path=resolve(root,'.'+new URL(req.url,'http://localhost').pathname);
    if(path!==root&&!path.startsWith(root+'/')){res.writeHead(403);res.end();return;}
    res.setHeader('Cross-Origin-Opener-Policy','same-origin');res.setHeader('Cross-Origin-Embedder-Policy','require-corp');
    const ext=extname(path);res.setHeader('Content-Type',ext==='.wasm'?'application/wasm':ext==='.json'?'application/json':ext==='.js'||ext==='.mjs'?'text/javascript':'text/html');
    res.end(ext?await readFile(path):'<!doctype html><title>Table I/O validation</title>');
  }catch{res.writeHead(404);res.end();}
}).listen(41739,'127.0.0.1');
