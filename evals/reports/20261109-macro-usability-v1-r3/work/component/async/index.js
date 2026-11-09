export {withTaskScope} from './scopes.mjs';
import {instantiate as create} from './component.js';
import {createHostImports,createDelivery} from './host.mjs';
import {Tasks} from './tasks.mjs';
export {TaskCancelledError,TaskTimeoutError} from './tasks.mjs';
const factories=[{"id":0,"kind":"future","unit":false}];
const asyncExports=["main"].map(name=>name.replace(/-([a-z0-9])/g,(_,c)=>c.toUpperCase()));
export async function instantiate(options={}) {
  if (typeof WebAssembly.Suspending!=='function' || typeof WebAssembly.promising!=='function') throw new Error('WebAssembly JSPI is required');
  const tasks=new Tasks({...options,asyncExports});
  
  const delivery=createDelivery(factories);
  const imports={
    ...createHostImports(options,tasks,[{"capability":"clock","effects":{"host":true,"suspends":false},"jsName":"deferI64","module":"wlt:host/event-loop@0.1.0","name":"defer-i64","params":["i32","i64"],"resource":null,"result":"(future i64)"}]),
    'wlt:runtime/delivery':delivery,'wlt:runtime/delivery@0.1.0':delivery,
    'wlt:runtime/tasks':tasks.children?.host()??{},'wlt:runtime/tasks@0.1.0':tasks.children?.host()??{},
  };
  let getCoreModule=options.getCoreModule;
  if (!getCoreModule && typeof process!=='undefined' && process.versions?.node) {
    getCoreModule=async name=>WebAssembly.compile(await (await import('node:fs/promises')).readFile(new URL(name,import.meta.url)));
  }
  const instantiateCore=async(module,imports)=>tasks.capture(await (options.instantiateCore??WebAssembly.instantiate)(module,imports));
  return tasks.bind(await create(getCoreModule,imports,instantiateCore,{wlt:{localCancellation:(e,id)=>tasks.active?.state!=="cancelling"&&!!tasks.active?.children?.scopes.get(id)?.cancelled&&e instanceof WebAssembly.Exception&&e.is(tasks.tag),scope:()=>tasks.scope,enter:id=>tasks.enter(id),read:tasks.read.bind(tasks),disposal:tasks.disposal.bind(tasks),cleaning:()=>tasks.cleaning===true,childrenContexts:clear=>{tasks.clearChildContexts=clear;}}}));
}
