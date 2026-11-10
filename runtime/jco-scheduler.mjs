// Pinned jco-transpile 0.23.0 polls suspended threads. A resumed thread must
// yield a real task turn before another tick, but does not need a clamped timer.
// Keep idle polling and one-thread-per-tick semantics unchanged.
const taskTurn = `
let walzTurnChannel;
const walzTurnQueue = [];
function walzSchedulerTurn() {
  if (typeof setImmediate === 'function') {
    return new Promise(resolve => setImmediate(resolve));
  }
  if (typeof MessageChannel === 'function') {
    return new Promise(resolve => {
      if (!walzTurnChannel) {
        walzTurnChannel = new MessageChannel();
        walzTurnChannel.port1.onmessage = () => {
          const pending = walzTurnQueue.splice(0);
          for (const resume of pending) resume();
        };
      }
      walzTurnQueue.push(resolve);
      if (walzTurnQueue.length === 1) walzTurnChannel.port2.postMessage(0);
    });
  }
  return new Promise(resolve => setTimeout(resolve, 0));
}
`;

export function tuneScheduler(source) {
  const marker =
    "const delay = result === ComponentAsyncState.TickResult.RESUMED ? 0 : 10;";
  if (!source.includes("class ComponentAsyncState") && !source.includes(marker))
    return source;
  const pattern =
    /const delay = result === ComponentAsyncState\.TickResult\.RESUMED \? 0 : 10;\s*await new Promise\(\(resolve\) => setTimeout\(resolve, delay\)\);/g;
  if ([...source.matchAll(pattern)].length !== 1)
    throw Error(
      "jco scheduler patch no longer matches; review the pinned transpiler",
    );
  source = source.replace(
    pattern,
    `if (result === ComponentAsyncState.TickResult.RESUMED) {
              await walzSchedulerTurn();
            } else {
              await new Promise(resolve => setTimeout(resolve, 10));
            }`,
  );
  // Module scope: the shared MessageChannel must never capture a component
  // instance. Queue entries retain continuations only until the next turn.
  const directive = /^"use components";/;
  return directive.test(source)
    ? source.replace(directive, (match) => match + "\n" + taskTurn)
    : taskTurn + "\n" + source;
}
