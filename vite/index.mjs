import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
import {createHash} from 'node:crypto';
import {mkdtemp, readFile, readdir, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {basename, join, resolve} from 'node:path';
import {declarations, writeDeclaration, removeDeclaration} from './types.mjs';

const execFile = promisify(callback);
const prefix = '\0wlt:';
const clock = 'wlt:host/event-loop@0.1.0';
const normalized = path => path.replaceAll('\\', '/');
const hash = value => createHash('sha256').update(value).digest('hex').slice(0, 24);

function request(id) {
  const [file, query=''] = id.split('?');
  if (!file.endsWith('.wlt')) return null;
  const params = new URLSearchParams(query);
  if (params.has('raw') || params.has('url')) return null;
  for (const key of params.keys()) {
    if (!['init', 'import', 't', 'v'].includes(key)) throw new Error(`Unsupported WLT query: ${key}`);
  }
  return {file, init:params.has('init')};
}

/** Compile checked WLT modules to the existing Core/JSPI boundary. */
export default function wlt(options={}) {
  const command = options.compiler ?? process.env.WLT ?? 'wlt';
  const argv = typeof command === 'string' ? [command] : command;
  if (!Array.isArray(argv) || !argv.length || argv.some(arg => typeof arg !== 'string' || !arg)) {
    throw new TypeError('compiler must be an executable or a nonempty array of executable and arguments');
  }
  const entries = new Map(), byKey = new Map(), running = new Set();
  let config, closed = false;

  function entry(file) {
    if (!entries.has(file)) {
      const value = {file, key:hash(file), dependencies:new Set([file]), epoch:0, compiled:null, pending:null, failed:false, ids:new Set()};
      entries.set(file, value); byKey.set(value.key, value);
    }
    return entries.get(file);
  }
  const virtual = (record, name) => `${prefix}${record.key}/${name}`;
  function artifact(id) {
    if (!id?.startsWith(prefix)) return null;
    const [key, name] = id.slice(prefix.length).split('/');
    const record = byKey.get(key);
    return record ? {record, name} : null;
  }
  function affected(file) {
    file = normalized(resolve(file));
    const found = [];
    for (const record of entries.values()) {
      if (record.dependencies.has(file) || (record.failed && /\.(wlt|wasm|wit|json)$/.test(file))) {
        record.epoch++; record.compiled = null; found.push(record);
      }
    }
    return found;
  }
  async function compile(record) {
    if (closed) throw new Error('WLT plugin has been closed');
    if (record.compiled) return record.compiled;
    if (record.pending) return record.pending;
    record.pending = (async () => {
      for (;;) {
        const epoch = record.epoch;
        const directory = await mkdtemp(join(tmpdir(), 'wlt-vite-build-'));
        const abort = new AbortController(); running.add(abort);
        const policy = options.bindings && resolve(config.root, options.bindings);
        try {
          if (policy) record.dependencies.add(normalized(policy));
          await execFile(argv[0], [...argv.slice(1), 'build', record.file, '--target', 'core-js', '-o', directory,
            ...(policy ? ['--bindings', policy] : [])],
          {cwd:config.root, signal:abort.signal, timeout:options.timeoutMs ?? 60000, maxBuffer:8*1024*1024});
          const manifest = JSON.parse(await readFile(join(directory, 'manifest.json'), 'utf8'));
          if (manifest.esm?.schema !== 'wlt-esm-v1' || !Array.isArray(manifest.sourceFiles)) {
            throw new Error('WLT CLI lacks ESM metadata; rebuild or install a matching wlt compiler');
          }
          const files = new Map();
          for (const name of await readdir(directory)) {
            if (/\.(m?js|wasm)$/.test(name)) files.set(name, await readFile(join(directory, name)));
          }
          if (!files.has('core.wasm') || !files.has('memory.wasm') || !files.has('index.js')) {
            throw new Error('WLT CLI produced an incomplete Core/JSPI distribution');
          }
          if (record.epoch !== epoch) continue;
          if (options.dts) await writeDeclaration(record.file, declarations(manifest.esm.exports));
          record.dependencies = new Set([record.file, ...manifest.sourceFiles.map(normalized), ...(policy ? [normalized(policy)] : [])]);
          record.failed = false;
          record.compiled = {files, manifest};
          return record.compiled;
        } catch (error) {
          if (record.epoch !== epoch && !closed) continue;
          record.failed = true;
          if (options.dts) await removeDeclaration(record.file);
          const message = error.code === 'ENOENT' ? `WLT compiler not found: ${argv[0]}. Install wlt or set the compiler option.`
            : error.stderr?.trim() || error.message;
          const failure = new Error(message, {cause:error});
          if (Number.isInteger(error.code) && !abort.signal.aborted && !closed) {
            // Ask the structured checker for original positions. Never recover
            // locations by parsing the compiler's rendered stderr.
            try {
              await execFile(argv[0], [...argv.slice(1), 'check', record.file, '--json',
                ...(policy ? ['--bindings', policy] : [])],
              {cwd:config.root, signal:abort.signal, timeout:options.timeoutMs ?? 60000, maxBuffer:8*1024*1024});
            } catch (checked) {
              try { failure.diagnostic = JSON.parse(checked.stdout).diagnostics?.[0]; } catch { /* retain the build error */ }
            }
          }
          throw failure;
        } finally { running.delete(abort); await rm(directory, {recursive:true, force:true}); }
      }
    })();
    try { return await record.pending; } finally { record.pending = null; }
  }
  function watch(context, record) {
    for (const path of record.dependencies) context.addWatchFile(path);
  }
  async function dispose() {
    closed = true;
    for (const abort of running) abort.abort();
    await Promise.allSettled([...entries.values()].map(record => record.pending));
    entries.clear(); byKey.clear();
  }

  return {
    name:'vite-plugin-wlt', enforce:'pre',
    configResolved(value) { config = value; closed = false; },
    async resolveId(source, importer) {
      if (source.startsWith(prefix)) return source;
      const parent = artifact(importer);
      // Every Vite factory supplies getCoreModule. Leave the unused native
      // fallback external instead of shipping a Node filesystem polyfill.
      if (source === 'node:fs/promises' && parent?.name === 'core.mjs'
          && this.environment.config.consumer === 'client') return {id:source, external:true};
      if (parent && source.startsWith('./')) {
        const name = source.slice(2);
        const compiled = await compile(parent.record);
        if (!compiled.files.has(name)) this.error(`Unknown generated WLT module ${source}`);
        return virtual(parent.record, name);
      }
      const parsed = request(source);
      if (!parsed) return null;
      const resolved = await this.resolve(parsed.file, importer, {skipSelf:true});
      if (!resolved || resolved.external) return null;
      return normalized(resolved.id.split('?')[0]) + (parsed.init ? '?init' : '');
    },
    async load(id) {
      const generated = artifact(id);
      const parsed = generated ? null : request(id);
      if (!generated && !parsed) return null;
      const record = generated?.record ?? entry(parsed.file);
      record.ids.add(id); watch(this, record);
      let compiled;
      try { compiled = await compile(record); } catch (error) {
        watch(this, record);
        const d = error.diagnostic, label = d?.primary;
        this.error({message:d ? [d.message, ...d.notes, ...d.help.map(help=>`help: ${help}`)].join('\n') : error.message,
          id:label?.location.path ?? record.file,
          ...(label ? {loc:{file:label.location.path ?? record.file, line:label.location.line, column:label.location.column-1}} : {})});
      }
      watch(this, record);
      const {manifest, files} = compiled;
      if (generated && generated.name !== 'factory.mjs') {
        const bytes = files.get(generated.name);
        if (!bytes || !/\.(m?js)$/.test(generated.name)) this.error(`Unknown generated WLT JavaScript module ${generated.name}`);
        return {code:bytes.toString('utf8'), map:null};
      }
      const factory = virtual(record, 'factory.mjs');
      if (generated) {
        const inline = config.command === 'serve' || this.environment.config.consumer === 'server';
        const binaries = ['core.wasm', 'memory.wasm'].map(name => {
          const bytes = files.get(name);
          const expression = inline ? JSON.stringify(bytes.toString('base64'))
            : `import.meta.ROLLDOWN_FILE_URL_${this.emitFile({type:'asset', name:`${basename(record.file, '.wlt')}.${name}`, source:bytes})}`;
          return `[${JSON.stringify(name)}, ${expression}]`;
        });
        return {code:`import {instantiate as create} from ${JSON.stringify(virtual(record, 'index.js'))};
const binaries = Object.fromEntries([${binaries.join(',')}]);
const modules = new Map();
async function load(name) {
  if (!Object.hasOwn(binaries, name)) throw new Error('Unknown WLT core module: '+name);
  if (!modules.has(name)) {
    const pending = (async()=>{
      ${inline ? "const bytes=Uint8Array.from(atob(binaries[name]), c=>c.charCodeAt(0));"
        : "const response=await fetch(binaries[name]);if(!response.ok)throw new Error('WLT Wasm request failed: '+response.status);const bytes=await response.arrayBuffer();"}
      return WebAssembly.compile(bytes);
    })();
    modules.set(name,pending);
    pending.catch(()=>{if(modules.get(name)===pending)modules.delete(name);});
  }
  return modules.get(name);
}
export function instantiate(options={}) {return create({...options,getCoreModule:options.getCoreModule??load});}
`, map:null};
      }
      if (parsed.init) {
        return {code:`export {instantiate as default, instantiate} from ${JSON.stringify(factory)};
export {TaskCancelledError, TaskTimeoutError} from ${JSON.stringify(virtual(record, 'index.js'))};`, map:null};
      }
      if (manifest.raceFree?.hostRequired) {
        this.error('WLT shared-memory host contracts require ?init and explicit instantiate options');
      }
      const namespaces = [...new Set([...(manifest.imports ?? []).map(b=>b.module), ...(manifest.memories ?? []).map(m=>m.module)])];
      const imports = [], providers = [];
      for (const namespace of namespaces) {
        const mapping = options.imports?.[namespace];
        if (!mapping && namespace === clock) continue;
        if (!mapping && namespace.startsWith('wlt:')) {
          this.error(`WLT host ${namespace} needs an imports mapping or explicit ?init options`);
        }
        const specifier = mapping?.startsWith('.') ? resolve(config.root, mapping) : mapping ?? namespace;
        const host = await this.resolve(specifier, record.file);
        if (!host) this.error(`Cannot resolve WLT host module ${namespace}; use an imports mapping or ?init`);
        const index = providers.length;
        imports.push(`import * as host${index} from ${JSON.stringify(host.id)};`);
        providers.push(`[${JSON.stringify(namespace)},{...host${index}}]`);
      }
      const names = new Set(), exports = [];
      for (const binding of Object.values(manifest.esm.exports)) {
        if (names.has(binding.jsName)) this.error(`Duplicate WLT JavaScript export: ${binding.jsName}`);
        names.add(binding.jsName);
        const index = exports.length;
        exports.push(`const e${index}=runtime.api[${JSON.stringify(binding.jsName)}];export {e${index} as ${JSON.stringify(binding.jsName)}};`);
      }
      return {code:`import {instantiate} from ${JSON.stringify(factory)};
${imports.join('\n')}
const runtime=await instantiate({imports:Object.fromEntries([${providers.join(',')}])});
${exports.join('\n')}`, map:null, moduleSideEffects:true};
    },
    hotUpdate({file, timestamp}) {
      const records = affected(file);
      if (!records.length) return;
      const graph = this.environment.moduleGraph;
      const seen = new Set();
      for (const record of records) for (const id of record.ids) {
        const module = graph.getModuleById(id);
        if (module) graph.invalidateModule(module, seen, timestamp, true);
      }
      if (this.environment.config.consumer === 'client') this.environment.hot.send({type:'full-reload'});
      return [];
    },
    watchChange(id) { if (config.command === 'build') affected(id); },
    closeServer:dispose,
    async closeBundle() { if (config.command === 'build' && !config.build.watch) await dispose(); },
  };
}
