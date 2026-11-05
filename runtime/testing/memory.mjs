// Test-only inspection of the pinned transpiler's live ownership tables.
// The callback is local to an instance: it neither keeps old instances alive
// nor includes payloads/results in the returned samples.
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
export async function instrument(
  directory,
  backend = process.env.GCWAT_BACKEND,
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
  if (source.split("class BufferManager {").length !== 2)
    throw new Error("jco buffer inspection anchor changed");
  source = source.replace(
    "class BufferManager {",
    "class BufferManager {\n  __gcwatSize(){return Array.from(this.#buffers.values()).reduce((n,b)=>n+b.size,0);}",
  );
  source = source.replace(marker, (match) =>
    match.replace(
      "return { api:",
      `return { __memory:()=>({
    scopes:RESOURCE_SCOPE_TASKS.size,bufferRegistry:BUFFER_MGR.__gcwatSize(),
    tasks:Array.from(ASYNC_TASKS_BY_COMPONENT_IDX.values()).reduce((n,t)=>n+t.length,0),
    futures:FUTURES.size(),streams:STREAMS.size(),
    futureEnds:Object.values(FUTURE_TABLES).reduce((n,x)=>n+x.table.size(),0),
    streamEnds:Object.values(STREAM_TABLES).reduce((n,x)=>n+x.table.size(),0),
    handles:Array.from(ASYNC_STATE.values()).reduce((n,s)=>n+s.handles.size(),0),
    threads:Array.from(ASYNC_STATE.values()).reduce((n,s)=>n+s.threads.size(),0),
    subtasks:Array.from(ASYNC_STATE.values()).reduce((n,s)=>n+s.subtasks.size(),0),
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
