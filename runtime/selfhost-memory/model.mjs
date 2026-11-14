// Fixed, finite regression criteria. RSS is an allocator/OS observation and is
// not substituted for retained live objects or backing storage.
export const limits=Object.freeze({heapUsed:2*1024*1024,arrayBuffers:1024*1024,external:2*1024*1024,embedder:2*1024*1024});
export function assess(samples,bounds=limits){
  if(samples.length<4)throw new Error('at least four post-warmup samples are required');
  if(!samples.every(s=>Number.isFinite(s.heapUsed)&&s.heapUsed>=0))
    throw new Error('every sample must include a finite nonnegative heapUsed measurement');
  const mean=(rows,key)=>rows.reduce((n,r)=>n+r[key],0)/rows.length;
  const growth={},failures=[];
  for(const key of ['heapUsed','arrayBuffers','external','embedder','rss']){
    if(!samples.every(s=>Number.isFinite(s[key])))continue;
    growth[key]=mean(samples.slice(-2),key)-mean(samples.slice(0,2),key);
    if(key in bounds&&growth[key]>bounds[key])failures.push({metric:key,growth:growth[key],limit:bounds[key]});
  }
  return {ok:failures.length===0,growth,failures};
}
