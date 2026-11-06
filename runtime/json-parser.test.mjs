import {test} from "node:test";
import assert from "node:assert/strict";
import {mkdtemp,rm,writeFile} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {pathToFileURL,fileURLToPath} from "node:url";
import {spawn} from "node:child_process";
const cli=fileURLToPath(new URL("./json-parser.mjs",import.meta.url));
function invoke(directory,input,file) {
  return new Promise((resolve,reject)=>{
    const child=spawn(process.execPath,[cli,directory,...(file?[file]:[])],{stdio:["pipe","pipe","pipe"]});
    let stdout="",stderr="";
    child.stdout.setEncoding("utf8").on("data",value=>stdout+=value);
    child.stderr.setEncoding("utf8").on("data",value=>stderr+=value);
    child.on("error",reject);child.on("close",code=>resolve({code,stdout,stderr}));
    child.stdin.on("error",error=>{if(error.code!=="EPIPE")reject(error);});
    child.stdin.end(input);
  });
}
for(const backend of ["core","component"])test(`${backend}: WLT JSON parser runs from stdin/files with byte-accurate diagnostics`,{timeout:60000},async()=>{
  const directory=await mkdtemp(join(tmpdir(),"wlt-json-cli-"));
  try {
    const {build}=await import(backend==="core"?"./build-core.mjs":"./build.mjs");
    await build("examples/json-parser/main.wlt",directory);
    const {instantiate}=await import(pathToFileURL(join(directory,"index.js")));
    const {api}=await instantiate();
    assert.equal(api.main(),42);
    const source='{\n "日本語": [true, null, "\\uD83E\\uDD80"],\n "n":9007199254740993,"small":1.2300e-2\n}';
    const expected='{"日本語":[true,null,"🦀"],"n":9007199254740993,"small":1.2300e-2}\n';
    assert.deepEqual(await invoke(directory,source),{code:0,stdout:expected,stderr:""});
    const input=join(directory,"input.json");await writeFile(input,source);
    assert.deepEqual(await invoke(directory,"",input),{code:0,stdout:expected,stderr:""});
    const bad='{\n "日本語": [1,]\n}';
    const error=await invoke(directory,bad);
    assert.equal(error.code,1);assert.equal(error.stdout,"");
    const offset=new TextEncoder().encode(bad.slice(0,bad.indexOf(']'))).length;
    assert.match(error.stderr,new RegExp(`byte ${offset}.*line 2, column 12`));
    assert.deepEqual(await invoke(directory,'null false'),{code:1,stdout:"",stderr:"JSON parse error at byte 5 (line 1, column 6)\n"});
    assert.deepEqual(await invoke(directory,'\ufeffnull'),{code:1,stdout:"",stderr:"JSON parse error at byte 0 (line 1, column 1)\n"});
    const limited=await invoke(directory,' '.repeat(65533)+'null');
    assert.equal(limited.code,1);assert.match(limited.stderr,/65536 bytes/);
    const utf8=await invoke(directory,Buffer.from([0x22,0xff,0x22]));
    assert.equal(utf8.code,1);assert.match(utf8.stderr,/UTF-8/);
    const missing=await invoke(directory,"",join(directory,"missing.json"));
    assert.equal(missing.code,1);assert.match(missing.stderr,/ENOENT/);
  } finally {await rm(directory,{recursive:true,force:true});}
});
