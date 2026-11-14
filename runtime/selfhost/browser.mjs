// Browser response adaptation; compilation and checking remain in the guest.
import {diagnosticMessage} from "./diagnostics.mjs";
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
export function selfhostDiagnostic(source,error){
  const {file,code,start,end}=error;
  const prefix=decoder.decode(encoder.encode(source).subarray(0,start));
  const lines=prefix.split('\n');
  return {code:`E_SELFHOST_${code}`,severity:'error',message:diagnosticMessage(code),
    primary:file===0?{span:{start,end},location:{path:'playground.walz',line:lines.length,column:[...lines.at(-1)].length+1},message:''}:null,
    related:[],notes:file===0?[]:[`walz:core byte range ${start}..${end}`],
    help:['The self-hosted compiler supports a subset of Walz. Select Rust for full language and tooling support.']};
}

export async function compileWithSelfhost(api,source){
  const files=[{path:'playground.walz',source:encoder.encode(source)}];
  const result=api.compileProject(files);
  if(result.tag==='err')return {schema:'walz-playground-v1',status:'error',diagnostic:selfhostDiagnostic(source,result.val)};
  const expansion=api.expandProject(files);
  return {schema:'walz-playground-v1',status:'success',wasm:result.val,
    sourceView:{format:'walz',text:expansion.tag==='ok'?decoder.decode(expansion.val):'Expanded Walz is unavailable for this source.'},
    analysis:{compiler:'selfhost',coverage:'validation-only',
      note:'The supported subset passed type, stack and effect validation. Detailed source analysis is available with the Rust compiler.'}};
}
