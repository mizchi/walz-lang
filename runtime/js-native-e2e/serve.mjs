import {createServer} from 'node:http';
import {mkdir,rm,readFile,copyFile,writeFile} from 'node:fs/promises';
import {resolve,join,extname} from 'node:path';
import {build as native} from '../build-js-native.mjs';
import {build as core} from '../build-core.mjs';
import {build as component} from '../build.mjs';
import {bundle} from '../flatbuffers-size.mjs';
const root=resolve('target/js-native-browser');await mkdir(root,{recursive:true});
for(const mode of ['utf8','js-strings']){
 const source=join(root,mode),output=join(root,mode+'-bundled');
 await native('examples/flatbuffers/main.walz',source,{mode});
 await native('tests/component-fixtures/js-native.walz',join(root,mode+'-values'),{mode});
 await native('tests/component-fixtures/utf8-views.walz',join(root,mode+'-utf8-views'),{mode});
 await rm(output,{recursive:true,force:true});await mkdir(output);
 const result=await bundle(`export {instantiate} from ${JSON.stringify(join(source,'index.js'))};`,'codec',output);
 await copyFile(join(source,'native.wasm'),join(output,'codec-js/native.wasm'));
 await writeFile(join(output,'entry.json'),JSON.stringify({entry:result.entry}));
}
for(const [name,build]of [['core',core],['component',component]]){
 await build('examples/flatbuffers/main.walz',join(root,name));
 await build('tests/component-fixtures/event-loop.walz',join(root,name+'-async'));
}
createServer(async(req,res)=>{
 res.setHeader('Cross-Origin-Opener-Policy','same-origin');res.setHeader('Cross-Origin-Embedder-Policy','require-corp');
 const path=new URL(req.url,'http://127.0.0.1').pathname;
 if(path==='/'){res.setHeader('Content-Type','text/html');res.end('<!doctype html><title>JS native experiment</title>');return;}
 const file=resolve(root,'.'+decodeURIComponent(path));
 if(!file.startsWith(root+'/')){res.writeHead(403);res.end();return;}
 try{const data=await readFile(file);res.setHeader('Content-Type',extname(file)==='.wasm'?'application/wasm':'text/javascript');res.end(data);}
 catch{res.writeHead(404);res.end();}
}).listen(41741,'127.0.0.1');
