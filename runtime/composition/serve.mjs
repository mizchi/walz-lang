import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve,extname} from 'node:path';
import {build} from '../build.mjs';
const root=resolve('target/composition-browser');
await build('examples/components/main.walz',root);
await build('examples/components/resources.walz',resolve(root,'resources'));
await build('examples/components/generics.walz',resolve(root,'generics'));
await build('examples/components/type-queries.walz',resolve(root,'queries'));
await build('examples/components/typed-constructors.walz',resolve(root,'typed-constructors'));
await build('tests/component-fixtures/composition-nominal-resources.walz',resolve(root,'nominal-resources'));
await build('tests/component-fixtures/composition-private-resources.walz',resolve(root,'private-resources'));
createServer(async(request,response)=>{
  try {
    const path=resolve(root,'.'+new URL(request.url,'http://localhost').pathname);
    if(!path.startsWith(root+'/')&&path!==root){response.writeHead(403).end();return;}
    const ext=extname(path);
    response.setHeader('Content-Type',ext==='.wasm'?'application/wasm':ext==='.js'?'text/javascript':'text/html');
    response.end(ext?await readFile(path):'<!doctype html><title>Component composition checks</title>');
  } catch {response.writeHead(404).end();}
}).listen(41742,'127.0.0.1');
