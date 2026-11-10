// Example adapter for a Walz program with cloneable numeric results. Its Future
// stays inside the worker; only the completed i64 crosses as a BigInt value.
export async function create({url}){
  let runtime=await (await import(url)).instantiate();
  return{
    async run({value,ms=0},{signal}){
      const task=runtime.start('double',[value,ms],{signal});
      try{return{value:await task.result};}
      finally{if(signal.aborted)await task.cancel(signal.reason);}
    },
    dispose(){runtime=null;},
  };
}
