import {QueryPool} from './pool.mjs';

const token=Symbol('FrozenIndex');
function word(value){if(!Number.isSafeInteger(value)||value<0||value>0xffffffff)throw new RangeError('query values must be unsigned 32-bit integers');return value;}
function pack(op,rows,width){
  rows=Array.from(rows);
  if(rows.length>524288)throw new RangeError('too many queries');
  const out=new Int32Array(rows.length*4);
  rows.forEach((row,i)=>{
    if(width===1)out.set([op,word(row),0,0],4*i);
    else{
      if(row.length!==width)throw new TypeError(`expected ${width} values per query`);
      const a=word(row[0]),b=word(row[1]),c=word(row[2]);
      if(op===3){const d=word(row[3]);if(c>d||d>256)throw new RangeError('value range must be within 0..256');out.set([op,a,b,c|(d<<16)],4*i);}
      else out.set([op,a,b,c],4*i);
    }
  });return out;
}
export const queries=Object.freeze({
  rank1:ends=>pack(0,ends,1),select1:ranks=>pack(1,ranks,1),select0:ranks=>pack(2,ranks,1),
  rangeFreq:ranges=>pack(3,ranges,4),quantile:ranges=>pack(4,ranges,3),
});

async function moduleAt(url) {
  const bytes=url.protocol==='file:'?await (await import('node:fs/promises')).readFile(url):await (await fetch(url)).arrayBuffer();
  return WebAssembly.compile(bytes);
}
export async function instantiate(base) {
  base=new URL(base);
  const serial=await import(new URL('serial/index.js',base));
  const {api}=await serial.instantiate();
  const module=await moduleAt(new URL('reader.wasm',base));
  return {
    buildBits(input,length){
      if(!(input instanceof Uint8Array)||!Number.isSafeInteger(length)||length<0||length>134000000||input.length!==Math.ceil(length/8))throw new RangeError('invalid bitvector input or logical length');
      return new FrozenIndex(api.buildBits(input,length),module,token);
    },
    buildWavelet(input){if(!(input instanceof Uint8Array))throw new TypeError('wavelet input must be Uint8Array');return new FrozenIndex(api.buildWavelet(input),module,token);},
  };
}
export class FrozenIndex {
  #bytes;#module;#memory=null;#pool=null;#workers=0;#closed=false;#closing=null;#creating=null;
  constructor(bytes,module,key){if(key!==token)throw new TypeError('use buildBits or buildWavelet');this.#bytes=bytes.slice();this.#module=module;}
  get byteLength(){return this.#bytes?.byteLength??0;}
  snapshot(){if(this.#closed)throw new Error('index is closed');return this.#bytes.slice();}
  async prepare({workers=1}={}) {
    if(this.#closed)throw new Error('index is closed');
    if(this.#pool){if(this.#workers!==workers)throw new Error('worker count is fixed after prepare');return;}
    if(this.#creating){await this.#creating;if(this.#workers!==workers)throw new Error('worker count is fixed after prepare');return;}
    this.#workers=workers;
    this.#creating=(async()=>{
      const memory=new WebAssembly.Memory({initial:256,maximum:256,shared:true});
      new Uint8Array(memory.buffer,4096,this.#bytes.length).set(this.#bytes);
      Atomics.store(new Int32Array(memory.buffer,0,2),1,1);
      this.#memory=memory;
      this.#pool=await QueryPool.create(this.#module,memory,this.#bytes.length,workers);
    })();
    try{await this.#creating;}finally{this.#creating=null;}
  }
  async queryMany(queries,{workers=1,signal}={}) {
    if(signal?.aborted)throw signal.reason??new DOMException('Aborted','AbortError');
    await this.prepare({workers});
    if(this.#closed)throw new Error('index is closed');
    return this.#pool.query(queries,{signal});
  }
  close(){
    if(this.#closing)return this.#closing;
    this.#closed=true;
    this.#closing=(async()=>{
      if(this.#creating)try{await this.#creating;}catch{}
      await this.#pool?.close();
      this.#pool=null;this.#memory=null;this.#bytes=null;this.#module=null;
    })().catch(error=>{this.#closing=null;throw error;});
    return this.#closing;
  }
  [Symbol.asyncDispose](){return this.close();}
}
