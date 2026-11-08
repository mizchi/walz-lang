import {createNodeFileSystem} from '../../runtime/node-filesystem.mjs';
import {FILESYSTEM} from '../../runtime/wasi-filesystem.mjs';
export default ()=>{
  const input=process.env.WLT_BENCH_INPUT;
  if(!input)throw new Error('Set WLT_BENCH_INPUT to the explicitly granted input file');
  const files=createNodeFileSystem({readFiles:{input}});
  return {imports:{[FILESYSTEM]:files},[Symbol.asyncDispose]:()=>files.close()};
};
