import {openStage} from "./selfhost/bridge.mjs";
import {compileWithSelfhost} from "./selfhost/browser.mjs";

let ready;
export function prepareSelfhost(){
  return ready??=load().catch(error=>{ready=undefined;throw error;});
}
export async function compileSelfhost(source){
  const api=await prepareSelfhost();
  return compileWithSelfhost(api,source);
}
async function load(){
  const metadataResponse=await fetch(new URL('./selfhost/build.json',import.meta.url),{cache:'no-cache'});
  if(!metadataResponse.ok)throw new Error(`Could not load self-hosted compiler metadata (${metadataResponse.status})`);
  const metadata=await metadataResponse.json();
  if(metadata.schema!=='walz-playground-selfhost-v1'||metadata.binaryEquality!==true||metadata.stage1Sha256!==metadata.sha256)
    throw new Error('Invalid self-hosted compiler build metadata');
  const response=await fetch(new URL('./selfhost/compiler.wasm',import.meta.url),{cache:'no-cache'});
  if(!response.ok)throw new Error(`Could not load the self-hosted compiler (${response.status})`);
  const binary=await response.arrayBuffer();
  const hash=[...new Uint8Array(await crypto.subtle.digest('SHA-256',binary))].map(n=>n.toString(16).padStart(2,'0')).join('');
  if(hash!==metadata.sha256)throw new Error('Self-hosted compiler hash mismatch; reload to obtain a consistent build');
  return openStage(binary);
}
