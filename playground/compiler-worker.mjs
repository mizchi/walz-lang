// Lazy initialization also makes a failed load report through the request.
let rustReady;
async function backend(engine){
  if(engine==='selfhost'){
    const compiler=await import('./selfhost-compiler.mjs');
    await compiler.prepareSelfhost();
    return {compile:compiler.compileSelfhost};
  }
  if(engine!=='rust')throw new Error('Unknown compiler: '+engine);
  const compiler=await(rustReady??=import('./pkg/walz_web.js').then(async compiler=>{
    await compiler.default();return compiler;
  }).catch(error=>{rustReady=undefined;throw error;}));
  return {compile:(source,optimize)=>JSON.parse(compiler.compile_source(source,optimize)),
    prepare:(source,optimize)=>JSON.parse(compiler.prepare_execution(source,optimize))};
}
self.onmessage = async ({ data: { id, source, optimize,engine='rust' } }) => {
  try {
    const compiler=await backend(engine);
    const start = performance.now();
    const result = await compiler.compile(source, optimize);
    result.engine=engine;
    result.durationMs = performance.now() - start;
    if (result.schema !== 'walz-playground-v1') throw new Error('Unexpected compiler response format');
    if (result.status === 'success') {
      result.wasm = Uint8Array.from(result.wasm);
      const transfer = [result.wasm.buffer];
      try {
        const module = await WebAssembly.compile(result.wasm);
        result.exports = WebAssembly.Module.exports(module).filter(item => item.kind === 'function').map(item => item.name);
        const imports = WebAssembly.Module.imports(module);
        if (compiler.prepare && !result.execution && imports.length && imports.every(item => item.module === 'walz:host/event-loop@0.1.0')) {
          const execution = compiler.prepare(source, optimize);
          if (execution.schema !== 'walz-playground-execution-v1') throw new Error('Unexpected execution response format');
          if (execution.status === 'success') {
            result.execution = execution.program;
          } else {
            result.executionError = execution.diagnostic.message;
          }
        }
        if(result.execution){
          result.exports=Object.keys(result.execution.exports);
          result.execution.wasm=Uint8Array.from(result.execution.wasm);
          result.execution.memory=Uint8Array.from(result.execution.memory);
          transfer.push(result.execution.wasm.buffer,result.execution.memory.buffer);
        }
      } catch (error) {
        // Compiler validation and browser feature support are independent.
        // Keep WAT/download available even if this engine cannot run the guest.
        result.exports = [];
        result.runtimeUnavailable = `This browser cannot run the generated Wasm: ${error.message}`;
      }
      self.postMessage({ id, result }, transfer);
    } else {
      self.postMessage({ id, result });
    }
  } catch (error) {
    self.postMessage({ id, error: String(error.message ?? error) });
  }
};
