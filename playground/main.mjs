import { WorkerClient } from './worker-client.mjs';
import { samples } from './samples.mjs';

const $ = id => document.getElementById(id);
const compiler = new WorkerClient(new URL('./compiler-worker.mjs', import.meta.url), { timeout: 30_000 });
const runner = new WorkerClient(new URL('./run-worker.mjs', import.meta.url), { timeout: 2_000, reuse: false });
let compiled;
let revision = 0;
let compiling = false;
let running = false;

function buttons() {
  $('compile').disabled = compiling;
  $('run').disabled = !compiled || running || !$('exports').value;
  $('download').disabled = !compiled;
  $('exports').disabled = !compiled || running;
  $('stop').disabled = !compiling && !running;
}

function tab(name) {
  for (const current of ['wat', 'analysis', 'diagnostics']) {
    const active = name === current;
    $(`tab-${current}`).setAttribute('aria-selected', String(active));
    $(`tab-${current}`).tabIndex = active ? 0 : -1;
    $(current).hidden = !active;
  }
}

function invalidate() {
  revision++;
  compiled = undefined;
  runner.cancel();
  $('exports').replaceChildren();
  for (const name of ['wat', 'analysis', 'diagnostics']) $(name).textContent = '';
  $('run-output').textContent = 'Run output appears here';
  $('compile-status').textContent = 'Changes pending · Compile to update';
  buttons();
}

function diagnosticText(diagnostic) {
  const label = item => {
    const { path, line, column } = item.location;
    return `${path ?? 'playground.wlt'}:${line}:${column}${item.message ? ` ${item.message}` : ''}`;
  };
  return [
    `${diagnostic.code}: ${diagnostic.message}`,
    ...(diagnostic.primary ? [label(diagnostic.primary)] : []),
    ...diagnostic.related.map(label),
    ...diagnostic.notes.map(note => `note: ${note}`),
    ...diagnostic.help.map(help => `help: ${help}`),
  ].join('\n');
}

async function compile() {
  if (compiling) return;
  invalidate();
  const current = revision;
  compiling = true;
  $('compile-status').textContent = 'Compiling…';
  buttons();
  try {
    const result = await compiler.run({ source: $('source').value, optimize: $('optimize').checked });
    if (current !== revision) return;
    if (result.status === 'error') {
      $('diagnostics').textContent = diagnosticText(result.diagnostic);
      $('compile-status').textContent = 'Compilation error';
      tab('diagnostics');
      return;
    }
    compiled = result;
    $('wat').textContent = result.wat;
    $('analysis').textContent = JSON.stringify(result.analysis, null, 2);
    $('diagnostics').textContent = result.runtimeUnavailable ?? 'No errors';
    for (const name of result.exports) $('exports').add(new Option(name, name));
    if (result.exports.includes('main')) $('exports').value = 'main';
    $('compile-status').textContent = `Compiled successfully · ${result.durationMs.toFixed(1)} ms · ${result.wasm.byteLength.toLocaleString('en-US')} bytes`;
    if (result.runtimeUnavailable) $('run-output').textContent = result.runtimeUnavailable;
    tab('wat');
  } catch (error) {
    if (current !== revision) return;
    $('diagnostics').textContent = error.message;
    $('compile-status').textContent = 'Could not complete compilation';
    tab('diagnostics');
    // A panic invalidates the Wasm instance; the next request loads it afresh.
    compiler.cancel();
  } finally {
    compiling = false;
    buttons();
  }
}

async function run() {
  if (!compiled || running) return;
  const current = revision;
  running = true;
  $('run-output').textContent = 'Running…';
  buttons();
  try {
    const wasm = compiled.wasm.slice();
    const execution = compiled.execution && {
      ...compiled.execution,
      wasm: compiled.execution.wasm.slice(),
      memory: compiled.execution.memory.slice(),
    };
    const transfer = [wasm.buffer];
    if (execution) transfer.push(execution.wasm.buffer, execution.memory.buffer);
    const result = await runner.run({ wasm, name: $('exports').value, execution, executionError: compiled.executionError }, transfer);
    if (current === revision) $('run-output').textContent = result;
  } catch (error) {
    if (current === revision) $('run-output').textContent = error.message;
  } finally {
    running = false;
    buttons();
  }
}

const groups = new Map();
for (const [name, sample] of Object.entries(samples)) {
  if (!groups.has(sample.category)) {
    const group = document.createElement('optgroup');
    group.label = sample.category;
    groups.set(sample.category, group);
    $('samples').append(group);
  }
  groups.get(sample.category).append(new Option(sample.label, name));
}
function selectSample() {
  const sample = samples[$('samples').value];
  $('source').value = sample.source;
  $('sample-title').textContent = sample.label;
  $('sample-description').textContent = sample.description;
  const expected = sample.expectation;
  $('sample-outcome').textContent = expected.kind === 'run' ? `Expected output: ${expected.output}`
    : expected.kind === 'compile' ? `Compile only · ${expected.note}`
    : `Expected diagnostic: ${expected.code}`;
  invalidate();
}
$('samples').onchange = selectSample;
selectSample();
$('source').addEventListener('input', () => {
  $('sample-title').textContent = 'Custom source';
  $('sample-description').textContent = 'Compile your changes to inspect the WAT, analysis and diagnostics.';
  $('sample-outcome').textContent = '';
  invalidate();
});
$('optimize').onchange = invalidate;
$('exports').onchange = buttons;
$('compile').onclick = compile;
$('run').onclick = run;
$('stop').onclick = () => {
  compiler.cancel();
  runner.cancel();
};
$('source').onkeydown = event => {
  if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') { event.preventDefault(); compile(); }
};
$('download').onclick = () => {
  if (!compiled) return;
  const url = URL.createObjectURL(new Blob([compiled.wasm], { type: 'application/wasm' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = 'program.wasm';
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1_000);
};
for (const [index, name] of ['wat', 'analysis', 'diagnostics'].entries()) {
  $(`tab-${name}`).onclick = () => tab(name);
  $(`tab-${name}`).onkeydown = event => {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? 2
      : (index + (event.key === 'ArrowRight' ? 1 : 2)) % 3;
    const name = ['wat', 'analysis', 'diagnostics'][next];
    tab(name);
    $(`tab-${name}`).focus();
  };
}
window.addEventListener('pagehide', () => { compiler.cancel(); runner.cancel(); });
compile();
