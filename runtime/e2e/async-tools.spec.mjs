import {test,expect} from '@playwright/test';
for(const backend of ['core','component']){
  test(`${backend}: WIT values and batch stream transport`,async({page})=>{
    await page.goto(`/${backend}/`);
    const result=await page.evaluate(async()=>{
      const load=name=>import(new URL(`${name}/index.js`,location.href).href);
      const {api:types}=await(await load('wit-values')).instantiate();
      const triple=types.triple([true,-128,'😀']),value=types.value({tag:'text',val:'漢😀'});
      const {api}=await(await load('stream-batch')).instantiate();let count=0,offset=0;
      let source={ [Symbol.asyncIterator](){return this;},async readMany(max){count++;const items=Array.from({length:Math.min(max,31-offset)},()=>offset++);return {items,done:offset===31};},async return(){return {done:true};} };
      const numbers=[];
      for(;;){const[tail,items]=await api.read(source,8);source=tail;if(!items.length)break;numbers.push(...items);}await source.return();
      const[empty,units]=await api.readUnit({[Symbol.asyncIterator](){return this;},async readMany(){return {items:[undefined,undefined],done:true};},async return(){return {done:true};}},8);await empty.return();
      return {triple,value,numbers,count,units:units.length,local:await api.local()};
    });
    expect(result).toEqual({triple:[true,-128,'😀'],value:{tag:'text',val:'漢😀'},numbers:Array.from({length:31},(_,i)=>i),count:4,units:2,local:42});
  });
  test(`${backend}: deadlines acknowledge disposal and keep outside calls alive`,async({page})=>{
    await page.goto(`/${backend}/`);
    const result=await page.evaluate(async()=>{
      const module=await import(new URL('deadlines/index.js',location.href).href);
      const {createEventLoop}=await import(new URL('deadlines/host.mjs',location.href).href);
      let disposed=0,allocator,guest;
      const {start}=await module.instantiate({eventLoop:{...createEventLoop(),sleep:()=>{const p=Promise.withResolvers();p.promise[Symbol.asyncDispose]=async()=>{await new Promise(r=>setTimeout(r,5));disposed++;p.resolve();};return p.promise;}},instantiateCore:async(m,imports)=>{const i=await WebAssembly.instantiate(m,imports);if(i.exports.live)allocator=i.exports;if(i.exports.__children_live)guest=i.exports;return i;}});
      const task=start('children',[60000],{timeoutMs:100});const outside=start('value');let deadline;
      try{await task.result;}catch(error){deadline=error.name;}
      const value=await outside.result;
      let grouped;
      try{await module.withTaskScope(start,async group=>group.start('children',[60000]).result,{timeoutMs:100});}catch(error){grouped=error.name;}
      return {deadline,grouped,value,disposed,live:allocator.live(),children:guest.__children_live()};
    });
    expect(result).toEqual({deadline:'TaskTimeoutError',grouped:'TaskTimeoutError',value:42,disposed:4,live:0,children:0});
  });
  test(`${backend}: WLT child cancellation isolates siblings and retires descendants`,async({page})=>{
    await page.goto(`/${backend}/`);
    const result=await page.evaluate(async()=>{
      const {instantiate}=await import(new URL('local-cancellation/index.js',location.href).href);let guest,memory;
      const {api}=await instantiate({instantiateCore:async(m,i)=>{const x=await WebAssembly.instantiate(m,i);if(x.exports.live)memory=x.exports;if(x.exports.__children_live)guest=x.exports;return x;}});
      const values=[];for(let n=0;n<3;n++)values.push([await api.run(),await api.cancel(),await api.running(),await api.nested(),await api.ready()]);
      const text='漢😀'.repeat(16384),delayed=(ms,v)=>new Promise(r=>setTimeout(()=>r(v),ms));
      const sibling=await api.text(delayed(8,'first'),delayed(15,text));await api.escaped();
      return {values,text:sibling,live:memory.live(),children:guest.__children_live()};
    });
    expect(result).toEqual({values:Array.from({length:3},()=>[42,42,undefined,undefined,21]),text:'漢😀'.repeat(16384),live:0,children:0});
  });

}
