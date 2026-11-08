// Node owns concrete handles; the language receives only text/result endpoints.
import { TEXT_IO, createTextIO } from "./text-io.mjs";
import { pathToFileURL } from "node:url";
import { resolve, join } from "node:path";

export function inputCapability(open) {
  let stream, iterator, closing;
  return {
    [Symbol.asyncIterator]() {
      return this;
    },
    next() {
      if (closing) return Promise.resolve({ done: true });
      if (!stream) {
        stream = open();
        iterator = stream[Symbol.asyncIterator]();
      }
      return iterator.next();
    },
    return() {
      return (closing ??= (async () => {
        if (!stream) return { done: true };
        const closed = stream.closed
          ? Promise.resolve()
          : new Promise((resolve) => stream.once("close", resolve));
        stream.destroy();
        await closed;
        // The stream is closed; the iterator may repeat the preceding read error.
        try {
          await iterator.return?.();
        } catch {}
        stream = iterator = undefined;
        return { done: true };
      })());
    },
  };
}

export function outputCapability(open, owned) {
  let stream, failure;
  const attach = () => {
    if (!stream) {
      stream = open();
      stream.on("error", capture);
    }
    return stream;
  };
  const capture = (error) => {
    failure = error;
  };
  return {
    write(bytes, signal) {
      if (signal.aborted) return Promise.resolve();
      const current = attach();
      if (failure) return Promise.reject(failure);
      return new Promise((resolve, reject) => {
        let settled = false;
        const finish = (error) => {
          if (settled) return;
          settled = true;
          signal.removeEventListener("abort", abort);
          current.off("close", close);
          current.off("error", failed);
          if (error) {
            failure = error;
            reject(error);
          } else resolve();
        };
        const close = () =>
          finish(signal.aborted ? undefined : new Error("output closed"));
        const failed = (error) => {
          if (!signal.aborted) finish(error);
        };
        const abort = () => {
          current.destroy();
          if (current.closed) finish();
        };
        current.once("close", close);
        current.once("error", failed);
        signal.addEventListener("abort", abort, { once: true });
        current.write(bytes, (error) => {
          // Destruction acknowledges an aborted write only after the close event.
          if (signal.aborted) {
            if (current.closed) finish();
          } else finish(error);
        });
        if (signal.aborted) abort();
      });
    },
    async close(abort) {
      // A selected output file must also be created/truncated for empty output.
      if (!stream && owned && !abort) attach();
      if (!stream) return;
      const current = stream,
        priorFailure = failure;
      try {
        if (owned || abort) {
          const closed = current.closed
            ? Promise.resolve()
            : new Promise((resolve) => current.once("close", resolve));
          if (abort || failure) current.destroy();
          else current.end();
          await closed;
        }
        if (failure && !priorFailure && !abort) throw failure;
      } finally {
        current.off("error", capture);
        stream = undefined;
      }
    },
  };
}

export async function runTextProgram(
  directory,
  {
    input = process.stdin,
    output = process.stdout,
    signal,
    onTaskEvent,
    maxLineBytes,
  } = {},
) {
  const source = inputCapability(
    typeof input === "function" ? input : () => input,
  );
  const sink = outputCapability(
    typeof output === "function" ? output : () => output,
    typeof output === "function",
  );
  let completed = false;
  try {
    const { instantiate } = await import(
      pathToFileURL(join(resolve(directory), "index.js"))
    );
    const { start } = await instantiate({
      imports: {
        [TEXT_IO]: createTextIO({
          input: source,
          write: sink.write,
          maxLineBytes,
        }),
      },
      onTaskEvent,
    });
    const result = await start("main", [], { signal }).result;
    completed = true;
    return result;
  } finally {
    const cleanup = await Promise.allSettled([
      source.return(),
      sink.close(!completed),
    ]);
    const errors = cleanup
      .filter((result) => result.status === "rejected")
      .map((result) => result.reason);
    if (errors.length) throw new AggregateError(errors, "I/O cleanup failed");
  }
}
