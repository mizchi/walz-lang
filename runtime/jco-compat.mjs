import { releaseEndpoints } from "./jco-lifetime.mjs";
import { tuneScheduler } from "./jco-scheduler.mjs";
import {batchStreams} from './jco-batches.mjs';
import {internalChildThreads} from './jco-children.mjs';
// Compatibility with pinned jco-transpile 0.23.0: payload-less endpoints emit
// null alignment but ManagedBuffer requires a number; its unit path also uses
// null despite the generated PromiseLike<void>/AsyncIterable<void> contract.
// Keep this patch local to generated glue. Integration tests exercise both
// directions for unit futures/streams, including genuinely pending reads.
function normalizeUnitBuffers(source) {
  if (!source.includes("class ManagedBuffer {")) return source;
  const replacements = [
    [
      "if (!args.elemMeta || typeof args.elemMeta.align32 !== 'number') {",
      "if (args.elemMeta?.isNone) args = {...args, elemMeta: {...args.elemMeta, align32: 0, size32: 0}};\n    if (!args.elemMeta || typeof args.elemMeta.align32 !== 'number') {",
    ],
    [
      "values = [...new Array(count)].map(() => null);",
      "values = [...new Array(count)].map(() => undefined);",
    ],
    [
      "if (!values.every(v => v === null)) {",
      "if (!values.every(v => v === null || v === undefined)) {",
    ],
  ];
  // Future-only glue omits StreamEnd and its iterator helpers entirely.
  if (source.includes("class StreamEnd"))
    replacements.push(
      // IteratorResult.done distinguishes EOF from a unit/option.none item.
      ["if (value === undefined) { return 0; }", ""],
      [
        "const appended = this.appendReadValue(res.value);",
        "const appended = res.done ? 0 : this.appendReadValue(res.value);",
      ],
    );
  for (const [before, after] of replacements) {
    if (source.split(before).length !== 2)
      throw new Error(
        "jco unit compatibility patch no longer matches; review the pinned transpiler",
      );
    source = source.replace(before, after);
  }
  return source;
}

export function adaptJspiGlue(source) {
  source = normalizeUnitBuffers(source);
  // A rejected imported async function has a parent subtask. In 0.23.0 its
  // completionPromise is never settled, so returning it after task.exit()
  // deadlocks the suspended caller. Propagate the original failure through
  // JSPI instead. Root export catch paths can propagate the same way.
  source = source.replaceAll(
    /task\.reject\(err\);\s*task\.exit\(\);\s*return task\.completionPromise\(\);/g,
    "task.reject(err);\n        task.exit();\n        throw err;",
  );
  // Expose pending-copy cancellation to our cooperative task scope. The copy
  // still completes through jco's own event/buffer protocol before guest unwind.
  for (const [kind, table, handle] of [
    ["future", "futureTableIdx", "futureEndWaitableIdx"],
    ["stream", "streamTableIdx", "streamEndWaitableIdx"],
  ]) {
    const declaration = `function ${kind}Read(`;
    if (!source.includes(declaration)) continue;
    source = source.replace(declaration, `function wltRaw${kind}Read(`);
    source += `
`; // Insert inside instantiate, where tables and options live.
    const endClass = kind === "future" ? "FutureEnd" : "StreamEnd";
    // Failed payload conversion leaves an abandoned copy buffer in 0.23.0.
    // Clear it before guest-scope cleanup drops the readable end.
    const wrapper = `function ${kind}Read(ctx,handle,...args){
      // Payload copies can complete on the producer's continuation. Stamp every
      // nested allocation with the waiting reader's scope before lowering it.
      const owner=options?.wlt?.scope?.(), getReallocFn=ctx.getReallocFn;
      if(owner!==undefined && getReallocFn)ctx={...ctx,getReallocFn:()=>{
        const realloc=getReallocFn();return realloc && ((...args)=>{options.wlt.enter(owner);return realloc(...args);});
      }};
      const end=get${kind[0].toUpperCase() + kind.slice(1)}End({tableIdx:ctx.${table},${handle}:handle});
      const failed=error=>{end.resetPendingBufferMeta();end.setCopyState(${endClass}.CopyState.DONE);throw error;};
      const run=()=>{try{const value=wltRaw${kind}Read(ctx,handle,...args);return value?.then?Promise.resolve(value).catch(failed):value;}catch(error){return failed(error);}};
      if(!options?.wlt?.read)return run();
      return options.wlt.read(()=>{if(end.isCopying()&&!end.hasPendingEvent()){end.__wltCancellingRead=true;end.cancel();}},run);
    }
`;
    source = source.replace(
      `function wltRaw${kind}Read(`,
      `${wrapper}function wltRaw${kind}Read(`,
    );
  }
  // Attach rejection handling before a synchronous read parks its task.
  // Waiting for a success event before observing failure deadlocks the reader.
  if(source.includes("class StreamEnd")) {
    const wakeup=/(const streamEnd = this;\n)([ \t]+)(return task\.suspendUntil\(\{)/g;
    if(!wakeup.test(source))throw new Error("jco failed-copy wakeup patch no longer matches");
    source=source.replace(wakeup,(_,head,indent,tail)=>`${head}${indent}if(injectedWritePromise)injectedWritePromise.catch(error=>streamEnd.setPendingEvent(()=>{throw error;}));
${indent}${tail}`);
  }
  // Task failure cleanup runs after jco has retired the caller task. Only
  // that private cleanup scope may drop readers without a current task.
  if(source.includes("function streamDropReadable(")) {
    const start=source.indexOf("function streamDropReadable(");
    const guard="if (!task) { throw new Error('invalid/missing async task'); }";
    const at=source.indexOf(guard,start);
    if(at<start || at>start+600)throw new Error("jco cleanup patch no longer matches; review pinned transpiler");
    source=source.slice(0,at)+guard.replace("if (!task)","if (!task && !options?.wlt?.cleaning?.())")+source.slice(at+guard.length);
  }
  const marker =
    "const reportErr = (err) => console.error('[jco] error while disposing discarded host value', err);";
  if (source.includes(marker))
    source = source.replace(
      marker,
      `if(options?.wlt?.disposal){options.wlt.disposal(()=>disposeFn.call(value));return;}
${marker}`,
    );
  // A cancelled copy has detached its guest buffer. Waiting for iterator.next()
  // here deadlocks a cancelled task whose producer waits for iterator.return().
  source = source.replaceAll(
    "if (deferSyncFinish) { await Promise.resolve(); }",
    `if (deferSyncFinish) { await Promise.resolve(); }
          if(this.__wltCancellingRead && injectedWritePromise){
            injectedWritePromise.then(cleanup=>cleanup(),()=>{});
            return finishCopy();
          }`,
  );
  return internalChildThreads(tuneScheduler(releaseEndpoints(batchStreams(source))));
}
