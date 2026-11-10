import {execFile as callback} from 'node:child_process';
import {promisify} from 'node:util';
import {createHash} from 'node:crypto';
import {readFile, readdir, realpath} from 'node:fs/promises';
import {basename, dirname, isAbsolute, join, resolve} from 'node:path';
import {projection} from '../vite/types.mjs';

const execFile = promisify(callback);
const canonical = async path => (await realpath(path).catch(async()=>join(await realpath(dirname(path)),basename(path)))).replaceAll('\\','/');
const failure = (messageText, code=9000, start=0, length=0) => ({text:'export {};\n',extension:'.ts',mappings:[],diagnostics:[{messageText,code,start,length}]});
const invalid = message => Object.assign(new Error(message),{code:-32602});

async function configuration(root, extras, argv) {
  const files = new Set(extras), seen = new Set();
  async function walk(directory) {
    directory=await canonical(directory);
    if (seen.has(directory)) return;
    seen.add(directory);
    for (const file of await readdir(directory,{withFileTypes:true})) {
      if (['node_modules','target','.git','.cache','dist'].includes(file.name) || file.isSymbolicLink()) continue;
      const path=join(directory,file.name);
      if (file.isDirectory()) await walk(path);
      else if (/\.(wlt|wit|wasm)$/.test(file.name)) files.add(path);
    }
  }
  await walk(root);
  const hash=createHash('sha256').update(JSON.stringify(argv));
  for (const path of [...files].sort()) {
    hash.update(path); hash.update('\0');
    try { hash.update(await readFile(path)); } catch (error) { if (error.code !== 'ENOENT') throw error; hash.update('missing'); }
    hash.update('\0');
  }
  return {configIdentity:hash.digest('hex'),watchedFiles:[...files].sort()};
}

/** Per-project options only; source snapshots and generated declarations are never cached. */
export function createMapper() {
  const projects=new Map(), running=new Set();
  let initialized=false;
  return {
    async handle(method, params={}) {
      if (method === 'initialize') {
        if (!params.positionEncodings?.includes('utf-8')) throw invalid('WLT Content Mapper requires UTF-8 positions');
        initialized=true;
        return {positionEncoding:'utf-8',diagnosticSource:'wlt'};
      }
      if (!initialized) throw invalid('Content Mapper must be initialized first');
      if (typeof params.projectHandle !== 'string' || !params.projectHandle) throw invalid('projectHandle must be a nonempty string');
      if (method === 'closeProject') { projects.delete(params.projectHandle); return null; }
      if (method === 'openProject') {
        if (typeof params.configFileName !== 'string' || (params.configFileName && !isAbsolute(params.configFileName))) throw invalid('configFileName must be absolute or empty');
        const base=params.configFileName ? dirname(params.configFileName) : process.cwd();
        const options=params.options ?? {}, optionDiagnostics=[];
        const report=(path,messageText)=>optionDiagnostics.push({path,messageText,code:1});
        const object=typeof options === 'object' && options !== null && !Array.isArray(options);
        if (!object) report([],'WLT mapper options must be an object');
        const values=object ? options : {};
        for (const [key,value] of Object.entries(values)) {
          if (!['compiler','timeoutMs','bindings','root','watchFiles'].includes(key)) report([key],`Unknown WLT mapper option: ${key}`);
          else if (['bindings','root'].includes(key) && (typeof value !== 'string' || !value)) report([key],`${key} must be a nonempty path`);
          else if (key === 'timeoutMs' && (!Number.isSafeInteger(value) || value <= 0)) report([key],'timeoutMs must be a positive integer');
          else if (key === 'watchFiles' && (!Array.isArray(value) || value.some(p=>typeof p !== 'string' || !p))) report([key],'watchFiles must be an array of paths');
          else if (key === 'compiler' && !(typeof value === 'string' && value || Array.isArray(value) && value.length && value.every(p=>typeof p === 'string' && p))) report([key],'compiler must be an executable or an array of executable and arguments');
        }
        const command=values.compiler ?? process.env.WLT ?? 'wlt';
        const argv=typeof command === 'string' ? [command] : command;
        const project={base,argv,timeout:values.timeoutMs ?? 60000,invalid:optionDiagnostics.length>0};
        projects.set(params.projectHandle,project);
        if (project.invalid) return {configIdentity:'invalid',optionDiagnostics};
        project.policy=values.bindings && resolve(base,values.bindings);
        const root=resolve(base,values.root ?? '.');
        const extra=[...(values.watchFiles ?? []).map(p=>resolve(base,p)),...(project.policy?[project.policy]:[])];
        project.configuration=await configuration(root,extra,argv);
        project.watched=new Set(await Promise.all(project.configuration.watchedFiles.map(p=>canonical(p).catch(()=>p))));
        return {...project.configuration,optionDiagnostics};
      }
      if (method !== 'transform') throw Object.assign(new Error(`Unknown method: ${method}`),{code:-32601});
      const project=projects.get(params.projectHandle);
      if (!project) throw invalid('Unknown or closed WLT project');
      if (project.invalid) return failure('Fix the invalid WLT mapper options before checking sources');
      if (typeof params.fileName !== 'string' || !isAbsolute(params.fileName) || typeof params.content !== 'string') throw invalid('transform requires an absolute fileName and string content');
      const abort=new AbortController(); running.add(abort);
      try {
        const fileName=await canonical(params.fileName);
        const pending=execFile(project.argv[0],[...project.argv.slice(1),'esm-metadata',fileName,'--stdin',
          ...(project.policy?['--bindings',project.policy]:[])],
        {cwd:project.base,signal:abort.signal,timeout:project.timeout,maxBuffer:16*1024*1024});
        pending.child.stdin.on('error',()=>{}); // A missing executable may close stdin before end().
        pending.child.stdin.end(params.content);
        let output;
        try { output=(await pending).stdout; } catch(error) { if (!error.stdout) throw error; output=error.stdout; }
        const report=JSON.parse(output);
        if (report.schema !== 'wlt-esm-metadata-v1') throw new Error('Install a matching wlt compiler with esm-metadata support');
        if (!report.ok) {
          const d=report.diagnostics[0], label=d.primary;
          const local=label?.location.path === fileName;
          const location=label?.location;
          const prefix=location && !local ? `${location.path}:${location.line}:${location.column}: ` : '';
          return failure(prefix+[`[${d.code}] ${d.message}`,...d.notes,...d.help.map(s=>`help: ${s}`)].join('\n'),9001,
            local?label.span.start:0,local?label.span.end-label.span.start:0);
        }
        // Changes to native WLT dependencies must also invalidate TS's cached
        // projection. Refuse unseen external inputs instead of returning stale types.
        const missing=report.sourceFiles.filter(path=>path!==fileName && !project.watched.has(path.replaceAll('\\','/')));
        if (missing.length) return failure(`WLT dependency outside mapper root; add to watchFiles: ${missing.join(', ')}`);
        return {...projection(report.esm.exports,fileName),diagnostics:[]};
      } catch(error) {
        return failure(error.code === 'ENOENT' ? `WLT compiler not found: ${project.argv[0]}. Install wlt or configure compiler.` : error.message);
      } finally { running.delete(abort); }
    },
    close() { for (const abort of running) abort.abort(); projects.clear(); },
  };
}
