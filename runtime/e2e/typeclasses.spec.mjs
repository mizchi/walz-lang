import {test,expect} from '@playwright/test';

for(const backend of ['core','component'])test(`${backend}: scalar type classes preserve widths, signed order and suspension`,async({page})=>{
  await page.goto(`/${backend}/`);
  const result=await page.evaluate(async()=>{
    const {instantiate}=await import(new URL('typeclasses/index.js',location.href).href);
    const {api}=await instantiate();
    const pair=api.pair(-1n);
    return {i32:api.add32(0x7fffffff,1),i64:String(api.add64(0x7fffffffffffffffn,1n)),
      f32:api.float32(16777216,1),f64:api.float64(1.5,40.5),less:api.less64(-1n,0n),
      eq:api.eq64(1n<<40n,0n),ne:api.ne32(1,2),literal:api.literals(),
      inferred:String(api.inferred(40n)),pair:[String(pair[0]),pair[1]],
      awaited:String(await api.awaitAdd(Promise.resolve(40n))),
      arithmetic:[api.arithmetic32(20,6,3,1),String(api.arithmetic64(20n,6n,3n,1n)),
        api.arithmeticF32(20,6,3,1),api.arithmeticF64(20,6,3,1)],
      integerQuotient:api.arithmetic32(7,0,1,-2),floatQuotient:api.arithmeticF64(7,0,1,-2),
      bits:api.bits32(0xf0,0x3c),bits64:String(api.bits64(1n<<60n,3n)),
      shifts:[api.shl32(1,32),api.ashift32(-1,1),api.ushift32(-1,1)],
      shifts64:[String(api.shl64(1n,64n)),String(api.ashift64(-1n,1n)),String(api.ushift64(-1n,1n))],
      traces:[api.logicalTrace(0,7,0),api.logicalTrace(2,7,0),api.logicalTrace(-1,7,1),api.logicalTrace(0,0,1)],
      skippedTrap:api.shortTrap(7),asyncSkipped:await api.asyncShort(1),asyncReached:await api.asyncShort(0)};
  });
  expect(result).toEqual({i32:-2147483648,i64:'-9223372036854775808',f32:16777216,f64:42,
    less:1,eq:0,ne:1,literal:4,inferred:'42',pair:['0',1],awaited:'42',
    arithmetic:[42,'42',42,42],integerQuotient:-3,floatQuotient:-3.5,
    bits:204,bits64:'1152921504606846979',shifts:[1,-1,2147483647],
    shifts64:['1','-1','9223372036854775807'],traces:[[0,1],[1,12],[1,1],[0,12]],
    skippedTrap:1,asyncSkipped:[1,0],asyncReached:[1,1]});
});
