// Same 512-bit directory and eight-level wavelet layout as Walz. This is the
// algorithm-matched JS competitor, not the independent unpacked test oracle.
export function popcount(x){x=x-((x>>>1)&0x55555555);x=(x&0x33333333)+((x>>>2)&0x33333333);return (((x+(x>>>4))&0x0f0f0f0f)*0x01010101)>>>24;}
const stride=n=>{const words=Math.ceil(n/32);return (words*4+(Math.ceil(words/16)+1)*4+15)&~15;};
function storage(kind,n){
  const size=64+stride(n)*(kind?8:1);
  if(!Number.isSafeInteger(n)||n<0||n>134000000||size>16773120)throw new RangeError('index exceeds limit');
  const bytes=new Uint8Array(size),u32=new Uint32Array(bytes.buffer);
  u32.set([1464611889,kind,n,size]);return {bytes,u32};
}
function directory(u32,base,n){
  const words=Math.ceil(n/32);let sum=0;
  for(let w=0;w<words;w++){if((w&15)===0)u32[base+words+(w>>>4)]=sum;sum+=popcount(u32[base+w]);}
  u32[base+words+Math.ceil(words/16)]=sum;
}
export class IndexedJS {
  #u32;#n;#kind;#stride;
  constructor(bytes){this.#u32=new Uint32Array(bytes.buffer,bytes.byteOffset,bytes.byteLength/4);this.#n=this.#u32[2];this.#kind=this.#u32[1];this.#stride=stride(this.#n)/4;}
  static bits(input,n){
    if(!Number.isSafeInteger(n)||n<0||input.length!==Math.ceil(n/8))throw new RangeError('invalid bit length');
    const {bytes,u32}=storage(0,n);bytes.set(input,64);
    if(n&31)u32[16+(n>>>5)]&=2**(n&31)-1;
    directory(u32,16,n);return new IndexedJS(bytes);
  }
  static wavelet(input){
    const n=input.length,{bytes,u32}=storage(1,n),step=stride(n)/4;
    let source=input.slice(),target=new Uint8Array(n);
    for(let level=0;level<8;level++){
      const base=16+level*step,shift=7-level;let zeros=0;
      for(let i=0;i<n;i++){const bit=(source[i]>>>shift)&1;u32[base+(i>>>5)]|=bit<<(i&31);zeros+=1-bit;}
      u32[4+level]=zeros;directory(u32,base,n);
      let z=0,o=zeros;for(let i=0;i<n;i++){const v=source[i];if((v>>>shift)&1)target[o++]=v;else target[z++]=v;}
      [source,target]=[target,source];
    }
    return new IndexedJS(bytes);
  }
  get byteLength(){return this.#u32.byteLength;}
  #rank(base,end){
    const n=this.#n,words=(n+31)>>>5,block=end>>>9,last=end>>>5,u=this.#u32;
    let sum=u[base+words+block];for(let w=block<<4;w<last;w++)sum+=popcount(u[base+w]);
    if(end&31)sum+=popcount(u[base+last]&(2**(end&31)-1));return sum;
  }
  #prefix(block,zero){const ones=this.#u32[16+((this.#n+31)>>>5)+block];return zero?Math.min(block*512,this.#n)-ones:ones;}
  #select(target,zero){
    const n=this.#n,blocks=Math.ceil(n/512);
    if(target>>>0>=this.#prefix(blocks,zero))return -1;
    let lo=0,hi=blocks;while(lo+1<hi){const mid=(lo+hi)>>>1;if(this.#prefix(mid,zero)<=target)lo=mid;else hi=mid;}
    let left=target-this.#prefix(lo,zero);
    for(let w=lo*16;;w++){
      const width=Math.min(32,n-w*32),mask=width===32?0xffffffff:2**width-1;
      let bits=((this.#u32[16+w]^(zero?-1:0))&mask)>>>0;const count=popcount(bits);
      if(left>=count){left-=count;continue;}
      for(let k=0;k<left;k++)bits&=bits-1;
      return w*32+31-Math.clz32(bits&-bits);
    }
  }
  #less(l,r,x){
    if(x>=256)return r-l;let sum=0;
    for(let level=0;level<8;level++){
      const base=16+level*this.#stride,a=this.#rank(base,l),b=this.#rank(base,r),zero=this.#u32[4+level];
      if((x>>>(7-level))&1){sum+=r-l-(b-a);l=zero+a;r=zero+b;}else{l-=a;r-=b;}
    }
    return sum;
  }
  #quantile(l,r,k){
    if(k>>>0>=r-l)return -1;let value=0;
    for(let level=0;level<8;level++){
      const base=16+level*this.#stride,a=this.#rank(base,l),b=this.#rank(base,r),count=r-l-(b-a),zero=this.#u32[4+level];
      value<<=1;if(k>=count){value|=1;k-=count;l=zero+a;r=zero+b;}else{l-=a;r-=b;}
    }
    return value;
  }
  queryMany(q){
    if(!(q instanceof Int32Array)||q.length%4)throw new TypeError('invalid queries');
    const out=new Int32Array(q.length/4),n=this.#n;
    for(let i=0,p=0;i<out.length;i++,p+=4){
      const op=q[p],a=q[p+1]>>>0,b=q[p+2]>>>0,c=q[p+3]>>>0;
      if(!this.#kind){
        if(op===0){if(a>n)throw new RangeError('rank end');out[i]=this.#rank(16,a);}
        else if(op===1||op===2)out[i]=this.#select(a,op===2);else throw new RangeError('query kind');
      }else{
        if(a>b||b>n)throw new RangeError('query range');
        if(op===3){const lo=c&65535,hi=c>>>16;if(lo>hi||hi>256)throw new RangeError('value range');out[i]=this.#less(a,b,hi)-this.#less(a,b,lo);}
        else if(op===4)out[i]=this.#quantile(a,b,c);else throw new RangeError('query kind');
      }
    }
    return out;
  }
}
