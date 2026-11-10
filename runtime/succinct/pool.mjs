import {WorkerHost} from '../workers/host.mjs';
export class QueryPool{
  #host;
  static async create(module,memory,length,count,{createWorker}={}){
    const pool=new QueryPool();
    pool.#host=await WorkerHost.create({workers:count,driver:new URL('./driver.mjs',import.meta.url),
      init:{module,memory,length,contract:'walz-readonly-regions-v1'},
      cancellation:{memory,byteOffset:0},...(createWorker?{workerFactory:createWorker}:{})});
    return pool;
  }
  get busy(){return this.#host.busy;}
  async query(queries,{signal}={}){
    if(!(queries instanceof Int32Array)||queries.length%4)throw new TypeError('queries must be Int32Array records of four words');
    if(queries.length/4>524288)throw new RangeError('query batch exceeds 524288 records');
    const count=queries.length/4,n=this.#host.size;
    const jobs=Array.from({length:n},(_,i)=>queries.slice(Math.floor(count*i/n)*4,Math.floor(count*(i+1)/n)*4));
    const results=await this.#host.map(jobs,{signal,transfer:q=>[q.buffer]});
    const out=new Int32Array(count);
    for(let i=0;i<n;i++){
      const start=Math.floor(count*i/n),end=Math.floor(count*(i+1)/n),result=results[i];
      if(!(result instanceof Int32Array)||result.length!==end-start)throw new Error('incomplete query batch');
      out.set(result,start);
    }return out;
  }
  close(){return this.#host.close();}
}
