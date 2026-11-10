// JSON grammar, AST and serialization live in Walz. This is only the CLI I/O.
import {createReadStream} from "node:fs";
import {resolve} from "node:path";
import {pathToFileURL} from "node:url";

const MAX_BYTES=65536;
async function readInput(file) {
  const input=file?createReadStream(file):process.stdin;
  const chunks=[];
  let size=0;
  for await(const chunk of input) {
    size+=chunk.length;
    if(size>MAX_BYTES)throw new Error(`JSON input exceeds ${MAX_BYTES} bytes`);
    chunks.push(chunk);
  }
  return Buffer.concat(chunks,size);
}
function location(text,offset) {
  // Parser offsets are UTF-8 bytes; displayed columns count Unicode scalars.
  const prefix=new TextDecoder().decode(new TextEncoder().encode(text).subarray(0,offset));
  const lines=prefix.split(/\r\n|\r|\n/);
  return {line:lines.length,column:[...lines.at(-1)].length+1};
}
export async function main(args) {
  if(args.length<1 || args.length>2)
    throw new Error("usage: json-parser.mjs output-directory [input.json]");
  const [directory,file]=args;
  const bytes=await readInput(file);
  let source;
  try {
    // Keep a BOM for the Walz parser to reject; do not silently strip input.
    source=new TextDecoder("utf-8",{fatal:true,ignoreBOM:true}).decode(bytes);
  } catch {
    throw new Error("JSON input is not valid UTF-8");
  }
  const {instantiate}=await import(pathToFileURL(resolve(directory,"index.js")));
  const {api}=await instantiate();
  const result=api.normalize(source);
  if(result.tag==="err") {
    const {line,column}=location(source,result.val);
    console.error(`JSON parse error at byte ${result.val} (line ${line}, column ${column})`);
    return 1;
  }
  process.stdout.write(result.val+"\n");
  return 0;
}
if(process.argv[1] && import.meta.url===pathToFileURL(resolve(process.argv[1])).href) {
  try {process.exitCode=await main(process.argv.slice(2));}
  catch(error) {console.error(error.message);process.exitCode=1;}
}
