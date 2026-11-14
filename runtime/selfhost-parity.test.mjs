import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, mkdir, writeFile, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, dirname} from 'node:path';
import {pathToFileURL} from 'node:url';
import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
import {build} from './build-core.mjs';
import {programs, projects, invalid, badProjects} from './selfhost-front/parity.mjs';
import * as budgets from './selfhost-front/budgets.mjs';
import * as integerBytes from './selfhost-front/bytes.mjs';
import * as scaling from './selfhost-front/scaling.mjs';
import * as serialization from './selfhost-front/serialization.mjs';
import * as tuning from './selfhost-front/tuning.mjs';
import * as abi from './selfhost-front/abi.mjs';
import * as control from './selfhost-front/control.mjs';
const exec=promisify(callback), bytes=s=>new TextEncoder().encode(s);

// Cargo coordinates reference invocations with concurrent compiler builds.
test('current frontend parity cases agree with Rust on execution and rejection',{timeout:300000},async()=>{
  const directory=await mkdtemp(join(tmpdir(),'walz-frontend-parity-'));
  try {
    const guest=join(directory,'compiler');await build('examples/selfhost/portable.walz',guest);
    const {api}=await(await import(pathToFileURL(join(guest,'index.js')))).instantiate();
    async function input(name,files) {
      const root=join(directory,name);
      for(const [path,source] of files){const target=join(root,path);await mkdir(dirname(target),{recursive:true});await writeFile(target,source);}
      return join(root,files[0][0]);
    }
    for(const [name,files,expected] of [...[...programs,...budgets.programs,...integerBytes.programs,...scaling.programs,...serialization.programs,...tuning.programs,...abi.programs,...control.programs].map(([n,s,e])=>[n,[['main.walz',s]],e]),...projects,...abi.projects]) {
      const entry=await input(name,files), binary=join(dirname(entry),'reference.wasm');
      const self=api.compileProject(files.map(([path,source])=>({path,source:bytes(source)})));
      assert.equal(self.tag,'ok',name+' '+JSON.stringify(self));
      assert.ok(WebAssembly.validate(self.val),name);
      await exec('cargo',['run','--locked','--quiet','--','build',entry,'--target','core','-o',binary]);
      for(const module of [self.val,await readFile(binary)]) {
        const {instance}=await WebAssembly.instantiate(module);
        if(expected==='trap') assert.throws(()=>instance.exports.f(),WebAssembly.RuntimeError,name);
        else assert.equal(instance.exports.f(),expected,name);
      }
    }
    for(const [name,files] of [...[...invalid,...budgets.invalid,...integerBytes.invalid,...scaling.invalid,...serialization.invalid,...abi.invalid,...control.invalid].map(([n,s])=>[n,[['main.walz',s]]]),...badProjects,...integerBytes.badProjects,...abi.badProjects]) {
      const entry=await input(name,files);
      assert.equal(api.checkProject(files.map(([path,source])=>({path,source:bytes(source)}))).tag,'err',name);
      let outcome;
      try {outcome=await exec('cargo',['run','--locked','--quiet','--','check','--json',entry]);}
      catch(error){outcome=error;}
      const diagnostic=JSON.parse(outcome.stdout);
      assert.equal(diagnostic.ok,false,name+' '+outcome.stdout);
      assert.ok(diagnostic.diagnostic?.code || diagnostic.diagnostics?.length,name+' must produce a checked diagnostic');
    }
  } finally {await rm(directory,{recursive:true,force:true});}
});
