import {instantiate as create} from './component.js';
import {createHostImports,createDelivery} from './host.mjs';
import {Tasks} from './tasks.mjs';
export {TaskCancelledError,TaskTimeoutError} from './tasks.mjs';
const factories=[];
const asyncExports=[].map(name=>name.replace(/-([a-z0-9])/g,(_,c)=>c.toUpperCase()));
export async function instantiate(options={}) {
  if (typeof WebAssembly.Suspending!=='function' || typeof WebAssembly.promising!=='function') throw new Error('WebAssembly JSPI is required');
  const tasks=new Tasks({...options,asyncExports});
  
  const delivery=createDelivery(factories);
  const imports={
    ...createHostImports(options,tasks,[]),
    'walz:runtime/delivery':delivery,'walz:runtime/delivery@0.1.0':delivery,
    'walz:runtime/tasks':tasks.children?.host()??{},'walz:runtime/tasks@0.1.0':tasks.children?.host()??{},
  };
  let getCoreModule=options.getCoreModule;
  if (!getCoreModule && typeof process!=='undefined' && process.versions?.node) {
    getCoreModule=async name=>WebAssembly.compile(await (await import('node:fs/promises')).readFile(new URL(name,import.meta.url)));
  }
  const instantiateCore=async(module,imports)=>tasks.capture(await (options.instantiateCore??WebAssembly.instantiate)(module,imports));
  return tasks.bind(await create(getCoreModule,imports,instantiateCore,{walz:{localCancellation:(e,id)=>tasks.active?.state!=="cancelling"&&!!tasks.active?.children?.scopes.get(id)?.cancelled&&e instanceof WebAssembly.Exception&&e.is(tasks.tag),scope:()=>tasks.scope,enter:id=>tasks.enter(id),read:tasks.read.bind(tasks),disposal:tasks.disposal.bind(tasks),cleaning:()=>tasks.cleaning===true,childrenContexts:clear=>{tasks.clearChildContexts=clear;}}}));
}
