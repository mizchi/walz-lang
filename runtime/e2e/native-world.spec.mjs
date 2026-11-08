import {test,expect} from '@playwright/test';
test('native WIT async producers and resource own/borrow work without the WLT SDK',async({page})=>{
 await page.goto('/');
 const result=await page.evaluate(async()=>{
  const load=async name=>{const url=new URL(`/native/${name}/native.js`,location.href);const {instantiate}=await import(url.href);return instantiate(async file=>WebAssembly.compile(await(await fetch(new URL(file,url))).arrayBuffer()),name==='resources'?{'test:resources/host':host}:{});};
  let closed=0;
  class File {constructor(){this.value=42;}[Symbol.dispose](){closed++;}}
  const host={File,make:()=>new File(),read:file=>file.value};
  const asyncApi=(await load('async'))['test:native/api'];const resourceApi=(await load('resources'))['test:resources/api'];
  const text=await(await asyncApi.readyText('日本😀'.repeat(20000)));const values=[];
  const stream=await asyncApi.prepend((async function*(){yield 8;yield 9;})());for await(const chunk of stream)values.push(...chunk);
  const f=resourceApi.make(),borrow=resourceApi.read(f),same=resourceApi.echo(f)===f;resourceApi.close(f);
  return {text,values,borrow,same,closed,ready:await(await asyncApi.ready()),consume:await asyncApi.consume(Promise.resolve(42))};
 });
 expect(result).toEqual({text:'日本😀'.repeat(20000),values:[7,8,9],borrow:42,same:true,closed:1,ready:42,consume:42});
});
