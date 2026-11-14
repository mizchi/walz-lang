// Public CDP commands routed to a dedicated worker through Playwright's root
// session. All evaluations return scalars/JSON; no RemoteObject is retained.
export async function attachWorker(root, targetId) {
  const { sessionId } = await root.send('Target.attachToTarget', { targetId, flatten: false });
  let nextId = 0;
  const pending = new Map();
  function receive(event) {
    if (event.sessionId !== sessionId) return;
    const message = JSON.parse(event.message), operation = pending.get(message.id);
    if (!operation) return;
    pending.delete(message.id); clearTimeout(operation.timer);
    if (message.error) operation.reject(new Error(message.error.message));
    else operation.resolve(message.result);
  }
  root.on('Target.receivedMessageFromTarget', receive);
  return {
    async send(method, params = {}) {
      const id = ++nextId;
      const result = new Promise((resolve, reject) => {
        const timer = setTimeout(() => { pending.delete(id); reject(new Error(`CDP timed out: ${method}`)); }, 30000);
        pending.set(id, { resolve, reject, timer });
      });
      await root.send('Target.sendMessageToTarget', { sessionId, message: JSON.stringify({ id, method, params }) });
      return result;
    },
    async close() {
      root.off('Target.receivedMessageFromTarget', receive);
      for (const operation of pending.values()) { clearTimeout(operation.timer); operation.reject(new Error('CDP session closed')); }
      pending.clear();
      await root.send('Target.detachFromTarget', { sessionId }).catch(() => {});
    },
  };
}

export async function evaluate(session, expression) {
  const response = await session.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
  if (response.exceptionDetails) throw new Error(JSON.stringify(response.exceptionDetails));
  return response.result.value;
}

export async function collect(session) {
  for (let i = 0; i < 4; i++) await session.send('HeapProfiler.collectGarbage');
  const memory = await session.send('Runtime.getHeapUsage');
  return { heapUsed: memory.usedSize, arrayBuffers: memory.backingStorageSize, embedder: memory.embedderHeapUsedSize };
}
