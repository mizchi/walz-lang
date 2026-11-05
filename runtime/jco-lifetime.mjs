// Lifetime corrections for the pinned jco-transpile 0.23.0 glue. Keep table
// retirement separate from drop notification: a late producer may still own
// direct references, but must no longer root a completed channel in the store.
function replaceOnce(source, before, after) {
  if (source.split(before).length !== 2)
    throw new Error(
      "jco lifetime patch no longer matches; review the pinned transpiler",
    );
  return source.replace(before, after);
}
export function releaseEndpoints(source) {
  if (source.includes("class BufferManager {")) {
    // The pinned glue passes buffer objects directly to copy continuations;
    // no runtime code ever looks them up by ID. The unused registry otherwise
    // roots every buffer forever, including its payload and guest memory.
    if (/BUFFER_MGR\.(getBuffer|deleteBuffer)\s*\(/.test(source))
      throw new Error(
        "jco now uses buffer lookup; review buffer lifetime handling",
      );
    source = replaceOnce(
      source,
      "instanceBuffers.set(nextBufID, buffer);",
      "// Copy continuations own the buffer; do not retain it in an unused registry.",
    );
  }
  if (
    !source.includes("class FutureEnd") &&
    !source.includes("class StreamEnd")
  )
    return source;
  let helpers = "";
  for (const kind of ["Future", "Stream"]) {
    if (!source.includes(`class ${kind}End`)) continue;
    const lower = kind.toLowerCase(),
      globals = kind === "Future" ? "FUTURES" : "STREAMS";
    helpers += `function wltRetire${kind}End(end){
      const detach = e=>{
        const meta=${kind.toUpperCase()}_TABLES[e.${lower}TableIdx()];
        const state=getOrCreateAsyncState(meta.componentIdx);
        if(state.handles.get(e.waitableIdx())===e)state.handles.remove(e.waitableIdx());
        if(meta.table.get(e.handle())===e)meta.table.remove(e.handle());
      };
      detach(end);
      const rep=end.global${kind}MapRep();
      const channel=${globals}.get(rep);
      // This language has host-owned writers. A closed reader will never
      // receive another value; pending producers check isPeerDropped first.
      if((channel?.readEnd?.()===end || channel?.writeEnd?.()===end) && channel.readEnd().isDropped()){
        detach(channel.writeEnd());
        ${globals}.remove(rep);
      }
    }
`;
    if (kind === "Future") {
      source = replaceOnce(
        source,
        "this.#dropped = true;\n    this.#notifyPeerDroppedFn();\n    this.#waitable.drop();",
        "this.#dropped = true;\n    this.#notifyPeerDroppedFn();\n    this.#waitable.drop();\n    wltRetireFutureEnd(this);",
      );
      source = replaceOnce(
        source,
        "return { value: vs[0] };",
        "this.drop();\n    return { value: vs[0] };",
      );
      // A program that only awaits host futures has no lifting helper. There
      // is no duplicate lifting registration to retire in that profile.
      if (source.includes("function futureNewFromLift("))
        source = replaceOnce(
          source,
          "const rep = FUTURES.insert(future);",
          "const rep = getFutureEnd({tableIdx:futureTableIdx,futureEndWaitableIdx}).globalFutureMapRep();",
        );
    } else {
      source = replaceOnce(
        source,
        "        w.drop();\n      }\n    }\n  }\n}\n\nfunction getStreamEnd",
        "        w.drop();\n      }\n    }\n    wltRetireStreamEnd(this);\n  }\n}\n\nfunction getStreamEnd",
      );
      source = replaceOnce(
        source,
        "const rep = STREAMS.insert(stream);",
        "const rep = getStreamEnd({tableIdx:streamTableIdx,streamEndWaitableIdx}).globalStreamMapRep();",
      );
      source = replaceOnce(
        source,
        "  #globalRep = null;",
        "  #globalRep = null;\n  #wltClosed = false;",
      );
      source = replaceOnce(
        source,
        "    return this.#readFn(readOpts);",
        `    if(this.#wltClosed)return {done:true};
    const result=await this.#readFn(readOpts);
    if(result.done)this[symbolDispose]();
    return result;`,
      );
      source = replaceOnce(
        source,
        "    this.#dropFn?.();",
        `    if(this.#wltClosed)return;
    this.#wltClosed=true;
    this.#dropFn?.();
    this.#dropFn=null;this.#readFn=null;this.#writeFn=null;`,
      );
    }
  }
  // Function declarations are instantiated per component, alongside jco tables.
  return replaceOnce(
    source,
    "  const WebAssembly =",
    helpers + "\n  const WebAssembly =",
  );
}
