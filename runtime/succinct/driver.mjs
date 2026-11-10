// The adapter knows the index layout. The general host knows only jobs/joins.
export async function create({module,memory,length,contract}){
  if(contract!=='walz-readonly-regions-v1')throw new Error('readonly host contract is required');
  let api=(await WebAssembly.instantiate(module,{env:{memory}})).exports;
  if(api.batch.length!==2)throw new Error('incompatible succinct worker kernel');
  api.init(length);
  return{
    run(queries,{workerId,isCancelled}){
      if(!(queries instanceof Int32Array)||queries.length%4)throw new TypeError('invalid succinct query records');
      const count=queries.length/4,offset=api.reserve(count);
      if(isCancelled())return{value:null};
      new Int32Array(api.output.buffer,0,queries.length).set(queries);
      if(api.batch(count,workerId)<0)return{value:null};
      const result=new Int32Array(api.output.buffer,offset,count).slice();
      return{value:result,transfer:[result.buffer]};
    },
    dispose(){api=null;},
  };
}
