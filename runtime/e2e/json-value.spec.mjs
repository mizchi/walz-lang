import {test,expect} from '@playwright/test';
for(const backend of ['core','component'])test(`${backend}: browser JSON parser and value construction`,async({page})=>{
  await page.goto(`/${backend}/`);
  const result=await page.evaluate(async()=>{
    const {instantiate}=await import(new URL('json-value/index.js',location.href).href);
    const {api}=await instantiate();
    const source='{"values":[null,true,1.25e2,"日本語\\uD83E\\uDD80"]}';
    const fastStrings=Array.from({length:34},(_,n)=>'a'.repeat(n)+'日本語🦀')
      .every(prefix=>['','\\n','\\"','\\\\','\\uD83E\\uDD80'].every(tail=>{
        const source='"'+prefix+tail+'"',result=api.normalize(source);
        return result.tag==='ok'&&JSON.parse(result.val)===JSON.parse(source);
      }));
    const errorOffsets=Array.from({length:34},(_,n)=>n).every(n=>{
      const result=api.normalize('"'+'a'.repeat(n)+'\u0001');
      return result.tag==='err'&&result.val===n+1;
    });
    const long=api.normalize('"'+'a'.repeat(48000)+'"');
    return {value:JSON.parse(api.normalize(source).val),constructed:JSON.parse(api.constructed().val),builder:JSON.parse(api.builder().val),error:api.normalize('[1,]'),depth128:api.normalize('['.repeat(128)+']'.repeat(128)).tag,depth129:api.normalize('['.repeat(129)+']'.repeat(129)).tag,precise:api.normalize('9007199254740993').val,fastStrings,errorOffsets,longString:long.tag==='ok'&&JSON.parse(long.val)==='a'.repeat(48000)};
  });
  expect(result).toEqual({value:{values:[null,true,125,'日本語🦀']},constructed:{values:['日本語',null],n:125},builder:{nothing:null,n:125,values:[null,true]},error:{tag:'err',val:3},depth128:'ok',depth129:'err',precise:'9007199254740993',fastStrings:true,errorOffsets:true,longString:true});
});
