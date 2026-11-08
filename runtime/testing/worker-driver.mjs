export async function create({bias,control}){
  return{
    async run({numbers,delay=0,bytes,cpu=false},{signal,workerId,isCancelled}){
      if(cpu){
        const words=new Int32Array(control.buffer);
        Atomics.store(words,1+workerId,1);Atomics.notify(words,1+workerId);
        while(!isCancelled()&&!Atomics.load(words,4+workerId)){}
        return{value:{cancelled:isCancelled(),workerId}};
      }
      if(delay)await new Promise((resolve,reject)=>{
        const done=()=>{signal.removeEventListener('abort',cancel);resolve();};
        const timer=setTimeout(done,delay);
        const cancel=()=>{clearTimeout(timer);signal.removeEventListener('abort',cancel);reject(signal.reason);};
        if(signal.aborted)cancel();else signal.addEventListener('abort',cancel,{once:true});
      });
      if(signal.aborted)throw signal.reason;
      if(bytes){const out=bytes.map(n=>n+bias);return{value:out,transfer:[out.buffer]};}
      return{value:numbers.reduce((a,b)=>a+b,bias)};
    },
    async dispose(){await Promise.resolve();},
  };
}
