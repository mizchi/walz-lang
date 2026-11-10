// Pinned jco 0.23.0's JSPI exports retain a task's exclusive lock while
// suspended. Walz children are internal threads of that same root task, not
// independent canonical calls (which would wait forever for the parent's lock).
// Keep separate thread metadata and retire it before acknowledging completion.
export function internalChildThreads(source) {
  if(!source.includes('async function walzChildRun('))return source;
  const pattern=/^([ \t]*)async function walzChildRun\(arg0, arg1\) \{/gm;
  const matches=[...source.matchAll(pattern)];
  if(matches.length!==1)throw new Error('jco child thread patch no longer matches');
  const header=matches[0],end=new RegExp('^'+header[1]+'}', 'gm');end.lastIndex=header.index+header[0].length;
  const closing=end.exec(source);
  if(!closing)throw new Error('jco child function boundary patch no longer matches');
  const body=source.slice(header.index,closing.index+closing[0].length), raw=body.match(/entryFnName: '(\w+ChildRun)'/)?.[1];
  const component=body.match(/componentIdx: (\d+)/)?.[1];
  const marker='() => _trackHostOperation(() => childNew(arg0))';
  if(!raw || !component || source.split(marker).length!==2 || !source.includes('class AsyncThread'))
    throw new Error('jco child context patch no longer matches');
  source=source.replace(body,"__WALZ_CHILD_BODY__");
  source=source.replace(marker,`() => _trackHostOperation(() => {
    const id=childNew(arg0), meta=_getGlobalCurrentThreadMeta(${component});
    const parent=getCurrentTask(${component},meta?.taskID)?.task;
    if(!parent)throw new Error('missing parent task for Walz child');
    walzChildContexts.set(id,parent);return id;
  })`);
  // The lowered sync-ABI async import parks after its host trampoline has
  // returned. That trampoline clears its metadata; restore the captured
  // caller thread while parking, rather than consulting another child's slot.
  const park='await task.suspendUntil({ readyFn: () => subtask.isResolved() });';
  if(source.split(park).length!==3)throw new Error('jco child suspension patch no longer matches');
  source=source.replaceAll(park,`await _withGlobalCurrentThreadMetaAsync({taskID:task.id(),threadID,componentIdx:task.componentIdx(),fn:()=>task.suspendUntil({readyFn:()=>subtask.isResolved()})});`);
  // An async import starts its host trampoline in a later microtask. Remember
  // the calling thread's subtask instead of taking the task-wide latest call.
  const push='this.#subtasks.push(newSubtask);';
  const latest='return this.#subtasks.at(-1);';
  if(source.split(push).length!==2 || source.split(latest).length!==3)
    throw new Error('jco child subtask patch no longer matches');
  source=source.replace(push,`${push}
    const meta=_getGlobalCurrentThreadMeta(this.#componentIdx);
    const thread=meta && getCurrentThread(this.#componentIdx,meta.threadID,this.#id)?.thread;
    newSubtask.__walzScope=options?.walz?.scope?.();
    if(thread){thread.__walzSubtask=newSubtask;newSubtask.__walzThread=thread;}`);
  source=source.replace(latest,`const meta=_getGlobalCurrentThreadMeta(this.#componentIdx);
    const thread=meta && getCurrentThread(this.#componentIdx,meta.threadID,this.#id)?.thread;
    return thread?.__walzSubtask ?? this.#subtasks.at(-1);`);
  const remove='this.#subtasks = this.#subtasks.filter(t => t !== subtask);';
  if(source.split(remove).length!==2)throw new Error('jco child subtask retirement patch no longer matches');
  source=source.replace(remove,`${remove}
    if(subtask.__walzThread?.__walzSubtask===subtask)delete subtask.__walzThread.__walzSubtask;
    delete subtask.__walzThread;`);
  const hostOperation='function _trackHostOperation(operation) {';
  if(source.split(hostOperation).length!==2)throw new Error('jco host scope patch no longer matches');
  source=source.replace(hostOperation,`${hostOperation}
    const meta=CURRENT_TASK_META.current;
    const owner=meta && getCurrentTask(meta.componentIdx,meta.taskID)?.task?.getParentSubtask()?.__walzScope;
    if(owner!==undefined)options?.walz?.enter?.(owner);`);
  const awaitedHost=/ret = await  _withGlobalCurrentThreadMetaAsync\(\{[\s\S]*?\}\)\s*;/g;
  if(![...source.matchAll(awaitedHost)].length)throw new Error('jco async result scope patch no longer matches');
  source=source.replace(awaitedHost,match=>`${match}
    if(subtask?.__walzScope!==undefined)options?.walz?.enter?.(subtask.__walzScope);`);
  const parentReject='this.#parentTask.reject(subtaskErr);';
  if(source.split(parentReject).length!==2)throw new Error('jco local cancellation patch no longer matches');
  source=source.replace(parentReject,`if(options?.walz?.localCancellation?.(subtaskErr,this.__walzScope)) {
    this.__walzError=subtaskErr;this.#parentTask.notifyProgress();
  } else {${parentReject}}`);
  const trap=`getOrCreateAsyncState(${component}).markTrapped(err);`;
  source=source.replaceAll(trap,`if(!options?.walz?.localCancellation?.(err,task.getParentSubtask()?.__walzScope)){${trap}}`);
  const resolved='if (subtask.isResolved()) { subtask.deliverResolve(); }';
  if(source.split(resolved).length!==3)throw new Error('jco local unwind patch no longer matches');
  source=source.replaceAll(resolved,`if(subtask.__walzError)throw subtask.__walzError; ${resolved}`);
  source=source.replace("__WALZ_CHILD_BODY__",`const walzChildContexts=new Map();
      options?.walz?.childrenContexts?.(()=>walzChildContexts.clear());
      async function walzChildRun(arg0,arg1) {
        const task=walzChildContexts.get(arg0);walzChildContexts.delete(arg0);
        if(!task)throw new Error('invalid Walz child context');
        const thread=task.addThread(new AsyncThread({componentIdx:${component},task}));
        try {
          return await _withGlobalCurrentThreadMetaAsync({componentIdx:${component},taskID:task.id(),threadID:thread.id(),fn:()=>${raw}(arg0,arg1)});
        } finally {task.removeThread(thread);}
      }`);
  // A subtask failure may reject a task completion before the parent Wasm
  // stack has unwound. The root must finish that stack before our runtime
  // acknowledges cancellation, clears GC slots, or reuses the ABI arena.
  return source.replaceAll('return await Promise.race([body, finishCompletion()]);',
    'const completion=finishCompletion();completion.catch(()=>{});await body;return await completion;');
}
