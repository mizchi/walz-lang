import {test,expect} from '@playwright/test';
for(const backend of ['core','component'])test(`${backend}: named struct construction and literal sugars`,async({page})=>{
  await page.goto(`/${backend}/`);
  const result=await page.evaluate(async()=>{
    const {instantiate}=await import(new URL('struct-construction/index.js',location.href).href);
    const {api}=await instantiate();
    return {profile:api.profile('Alice',30,'alice@example.com'),literal:api.literalProfile(),boxed:String(api.boxed()),shorthand:String(api.shorthand(41n)),
      order:api.order(),normal:api.throwBeforeLater(0),thrown:api.throwBeforeLater(1),
      early:api.earlyReturn(1),branch:api.branch(),awaited:await api.awaitSum(Promise.resolve(2))};
  });
  expect(result).toEqual({profile:['Alice',[30,'alice@example.com']],literal:['日本語🦀',[30,'alice@example.com']],
    boxed:'42',shorthand:'42',order:2112,normal:231,thrown:2,early:42,branch:42,awaited:42});
});
