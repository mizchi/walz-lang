// Test-only inspection of the pinned transpiler's live ownership tables.
// The callback is local to an instance: it neither keeps old instances alive
// nor includes payloads/results in the returned samples.
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
export async function instrument(
  directory,
  backend = process.env.WALZ_BACKEND,
) {
  if (backend === "core") {
    const path = join(directory, "core.mjs");
    const source = await readFile(path, "utf8");
    const marker = "return tasks.bind({ api });";
    if (source.split(marker).length !== 2)
      throw new Error("Core memory inspection anchor changed");
    await writeFile(
      path,
      source.replace(
        marker,
        "return tasks.bind({api,__memory:()=>({handles:boundary.handles.size})});",
      ),
    );
    return;
  }
  const path = join(directory, "component.js");
  let source = await readFile(path, "utf8");
  const marker = /^\s*return \{ api:.*$/m;
  if ([...source.matchAll(new RegExp(marker, "gm"))].length !== 1)
    throw new Error("jco memory inspection anchor changed");
  // The pinned generator omits registries when their types are not reachable.
  if (source.includes("BUFFER_MGR")) {
    if (source.split("class BufferManager {").length !== 2)
      throw new Error("jco buffer inspection anchor changed");
    source = source.replace(
      "class BufferManager {",
      "class BufferManager {\n  __walzSize(){return Array.from(this.#buffers.values()).reduce((n,b)=>n+b.size,0);}",
    );
  }
  source = source.replace(marker, (match) =>
    match.replace(
      "return { api:",
      `return { __memory:()=>({
    scopes:typeof RESOURCE_SCOPE_TASKS==='undefined'?0:RESOURCE_SCOPE_TASKS.size,
    bufferRegistry:typeof BUFFER_MGR==='undefined'?0:BUFFER_MGR.__walzSize(),
    tasks:typeof ASYNC_TASKS_BY_COMPONENT_IDX==='undefined'?0:Array.from(ASYNC_TASKS_BY_COMPONENT_IDX.values()).reduce((n,t)=>n+t.length,0),
    futures:typeof FUTURES==='undefined'?0:FUTURES.size(),
    streams:typeof STREAMS==='undefined'?0:STREAMS.size(),
    futureEnds:typeof FUTURE_TABLES==='undefined'?0:Object.values(FUTURE_TABLES).reduce((n,x)=>n+x.table.size(),0),
    streamEnds:typeof STREAM_TABLES==='undefined'?0:Object.values(STREAM_TABLES).reduce((n,x)=>n+x.table.size(),0),
    handles:typeof ASYNC_STATE==='undefined'?0:Array.from(ASYNC_STATE.values()).reduce((n,s)=>n+s.handles.size(),0),
    threads:typeof ASYNC_STATE==='undefined'?0:Array.from(ASYNC_STATE.values()).reduce((n,s)=>n+s.threads.size(),0),
    subtasks:typeof ASYNC_STATE==='undefined'?0:Array.from(ASYNC_STATE.values()).reduce((n,s)=>n+s.subtasks.size(),0),
  }), api:`,
    ),
  );
  await writeFile(path, source);
}
export const tick = () => new Promise((r) => setImmediate(r));
export async function collect() {
  if (typeof global.gc !== "function")
    throw new Error("memory checks require node --expose-gc");
  for (let i = 0; i < 4; i++) {
    await tick();
    global.gc();
  }
  await tick();
  const { heapUsed, external, arrayBuffers, rss } = process.memoryUsage();
  return { heapUsed, external, arrayBuffers, rss };
}

// Test artifacts only: no references/payloads escape through these snapshots.
export async function instrumentScopes(directory) {
  const path = join(directory, "tasks.mjs");
  const source = await readFile(path, "utf8");
  const marker = "      ...visible,\n      api,";
  if (source.split(marker).length !== 2)
    throw new Error("task scope inspection anchor changed");
  await writeFile(path, source.replace(marker, `      ...visible,
      __taskMemory:()=>({
        resources:this.resources.entries.size, queued:this.queue.length,
        active:this.active===null?0:1,
        reads:this.active?.reads.size??0,
        blocking:this.active?.blocking.size??0,
        disposals:this.active?.disposals.length??0,
        children:this.active?.children?.jobs.size??0,
      }),
      api,`));
}
