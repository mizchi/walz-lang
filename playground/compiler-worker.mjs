import init, { compile_source, prepare_execution } from './pkg/wlt_web.js';

// Lazy initialization also makes a failed load report through the request.
let ready;
self.onmessage = async ({ data: { id, source, optimize } }) => {
  try {
    await (ready ??= init());
    const start = performance.now();
    const result = JSON.parse(compile_source(source, optimize));
    result.durationMs = performance.now() - start;
    if (result.schema !== 'wlt-playground-v1') throw new Error('Unexpected compiler response format');
    if (result.status === 'success') {
      result.wasm = Uint8Array.from(result.wasm);
      const transfer = [result.wasm.buffer];
      try {
        const module = await WebAssembly.compile(result.wasm);
        result.exports = WebAssembly.Module.exports(module).filter(item => item.kind === 'function').map(item => item.name);
        const imports = WebAssembly.Module.imports(module);
        if (imports.length && imports.every(item => item.module === 'wlt:host/event-loop@0.1.0')) {
          const execution = JSON.parse(prepare_execution(source, optimize));
          if (execution.schema !== 'wlt-playground-execution-v1') throw new Error('Unexpected execution response format');
          if (execution.status === 'success') {
            result.execution = execution.program;
            result.execution.wasm = Uint8Array.from(execution.program.wasm);
            result.execution.memory = Uint8Array.from(execution.program.memory);
            transfer.push(result.execution.wasm.buffer, result.execution.memory.buffer);
          } else {
            result.executionError = execution.diagnostic.message;
          }
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
