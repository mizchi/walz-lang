import { instantiate } from './runtime/core.mjs';

async function executeHost(program, name) {
  if (typeof WebAssembly.Suspending !== 'function' || typeof WebAssembly.promising !== 'function') {
    throw new Error('This browser needs WebAssembly JSPI to run event-loop imports');
  }
  const entry = program.exports[name];
  if (!entry) throw new Error('Select an exported function to run');
  if (entry.arity !== 0) throw new Error(`Select a function with no arguments (${entry.arity} argument${entry.arity === 1 ? '' : 's'} required)`);
  if (!program.manifest.imports.every(item => item.module === 'walz:host/event-loop@0.1.0')) {
    throw new Error('Only event-loop host imports are available in this playground');
  }
  const url = URL.createObjectURL(new Blob([program.bindings], { type: 'text/javascript' }));
  try {
    const { bind } = await import(url);
    const Children=program.manifest.childTasks?(await import('./runtime/children.mjs')).Children:null;
    const readables=program.manifest.readables?(await import('./runtime/readables.mjs')).install:null;
    const { api } = await instantiate(import.meta.url, program.manifest.abi, bind, {
      getCoreModule: file => {
        if (file === 'core.wasm') return WebAssembly.compile(program.wasm);
        if (file === 'memory.wasm') return WebAssembly.compile(program.memory);
        throw new Error(`Unknown runtime module: ${file}`);
      },
    }, program.manifest.imports,program.manifest.memories??[],program.manifest.raceFree??null,Children,readables);
    return await api[entry.js_name]();
  } finally { URL.revokeObjectURL(url); }
}

async function executeRaw(wasm, name) {
  const module = await WebAssembly.compile(wasm);
  const imports = WebAssembly.Module.imports(module);
  if (imports.length) {
    throw new Error(`Host imports are not available in this playground: ${imports.map(item => `${item.module}.${item.name}`).join(', ')}`);
  }
  const instance = await WebAssembly.instantiate(module);
  const entry = instance.exports[name];
  if (typeof entry !== 'function') throw new Error('Select an exported function to run');
  if (entry.length !== 0) throw new Error(`Select a function with no arguments (${entry.length} argument${entry.length === 1 ? '' : 's'} required)`);
  return entry();
}

self.onmessage = async ({ data: { id, wasm, name, execution, executionError } }) => {
  try {
    if (executionError) throw new Error(`Cannot prepare event-loop execution: ${executionError}`);
    const result = execution ? await executeHost(execution, name) : await executeRaw(wasm, name);
    const display = value => typeof value === 'bigint' ? `${value}n`
      : value === undefined ? 'unit' : value === null ? 'null'
      : Array.isArray(value) ? `[${value.map(display).join(', ')}]` : String(value);
    self.postMessage({ id, result: display(result) });
  } catch (error) {
    self.postMessage({ id, error: String(error.message ?? error) });
  }
};
