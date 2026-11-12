import {test,expect} from '@playwright/test';
for(const mode of ['utf8','js-strings'])for(const bundled of [false,true])test(`${mode}: ${bundled?'bundled':'direct'} native ABI preserves bytes and JS strings`,async({page})=>{
 const errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto('/');
 const result=await page.evaluate(async({mode,bundled})=>{
  let path=mode+'/index.js';if(bundled){const {entry}=await(await fetch(mode+'-bundled/entry.json')).json();path=mode+'-bundled/'+entry;}
  const {instantiate}=await import(new URL(path,location.href));const p=await instantiate(),{api}=p;
  const input={id:42n,name:'\ufeff日本🦀',values:Int32Array.of(-1,42),payload:Uint8Array.of(0,128,255),enabled:false,weight:-1.25,child:{count:42,label:'child'}};
  const value=api.decode(api.encode(input)),invalid=api.decode(new Uint8Array());
  const {instantiate:values}=await import(new URL(mode+'-values/index.js',location.href));const {api:v}=await values();
  return {main:api.main(),id:value.val.id.toString(),name:value.val.name,values:Array.from(value.val.values),payload:Array.from(value.val.payload),invalid,
   echo:v.echo('A😀'),length:v.length('A😀'),literal:v.literal(),utf8:v.fromBytes(Uint8Array.of(255)),bom:v.fromBytes(Uint8Array.of(239,187,191,65)),
   start:typeof p.start,handles:p.__memory().hostHandles};
 },{mode,bundled});
 expect(result).toEqual({main:42,id:'42',name:'\ufeff日本🦀',values:[-1,42],payload:[0,128,255],invalid:{tag:'err',val:1},echo:'A😀',length:5,literal:'\ufeff日本🦀',utf8:undefined,bom:'\ufeffA',start:'undefined',handles:0});expect(errors).toEqual([]);
});
for(const backend of ['core','component'])test(`${backend}: portable codec, JSPI and cancellation retain their existing protocol`,async({page})=>{
 await page.goto('/');const result=await page.evaluate(async backend=>{
  const {instantiate}=await import(new URL(backend+'/index.js',location.href));const codec=await instantiate();
  const {instantiate:asyncInstance}=await import(new URL(backend+'-async/index.js',location.href));const p=await asyncInstance();
  let timer=false;const running=p.api.main(20);setTimeout(()=>timer=true,0);const answer=await running;
  const task=p.start('hold',[60000]);await new Promise(r=>setTimeout(r,10));const rejected=task.result.then(()=>false,e=>e.name);
  const done=await task.cancel();return {codec:codec.api.main(),answer:answer.toString(),timer,cancelled:await rejected,status:done.status};
 },backend);expect(result).toEqual({codec:42,answer:'42',timer:true,cancelled:'TaskCancelledError',status:'cancelled'});
});
