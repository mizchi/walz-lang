// Value-only text capabilities. Providers own their I/O and implement close.
export const TEXT_IO = "gcwat:host/text-io@0.1.0";
export const IOError = Object.freeze({
  utf8: 1,
  read: 2,
  write: 3,
  limit: 4,
  closed: 5,
});
const done = () => ({ done: true });
const ok = (val) => ({ tag: "ok", val });
const err = (val) => ({ tag: "err", val });
const stopped = Symbol("closed input");

function lines(input, maxLineBytes, notify) {
  let source = (input[Symbol.asyncIterator] ?? input[Symbol.iterator])?.call(
    input,
  );
  if (typeof source?.next !== "function")
    throw new TypeError("expected byte iterable");
  let buffer = new Uint8Array(Math.min(1024, maxLineBytes)),
    length = 0;
  let chunk,
    offset = 0,
    closed = false,
    reading = false,
    closing;
  const stop = Promise.withResolvers();
  const decoder = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
  const close = () => {
    if (closing) return closing;
    closed = true;
    stop.resolve(stopped);
    const current = source;
    source = chunk = buffer = undefined;
    length = offset = 0;
    closing = Promise.resolve()
      .then(() => current?.return?.())
      .then(() => undefined)
      .finally(() => notify({ type: "input-close" }));
    // close can also be reached from next(); its caller observes the failure.
    closing.catch(() => {});
    return closing;
  };
  const fail = async (code) => {
    await close();
    return { done: false, value: err(code) };
  };
  const decode = (crlf) => {
    const end =
      crlf && length && buffer[length - 1] === 13 ? length - 1 : length;
    const value = decoder.decode(buffer.subarray(0, end));
    length = 0;
    return value;
  };
  return {
    [Symbol.asyncIterator]() {
      return this;
    },
    async next() {
      if (closed) return done();
      if (reading) throw new TypeError("concurrent line read");
      reading = true;
      try {
        while (!closed) {
          if (chunk && offset < chunk.length) {
            const lf = chunk.indexOf(10, offset),
              end = lf < 0 ? chunk.length : lf;
            const size = length + end - offset;
            if (size > maxLineBytes) return await fail(IOError.limit);
            if (size > buffer.length) {
              const next = new Uint8Array(
                Math.min(maxLineBytes, Math.max(size, buffer.length * 2)),
              );
              next.set(buffer.subarray(0, length));
              buffer = next;
            }
            buffer.set(chunk.subarray(offset, end), length);
            length = size;
            offset = lf < 0 ? end : lf + 1;
            if (lf >= 0) {
              let value;
              try {
                value = decode(true);
              } catch {
                return await fail(IOError.utf8);
              }
              return { done: false, value: ok(value) };
            }
            chunk = undefined;
          }
          let result;
          try {
            result = await Promise.race([source.next(), stop.promise]);
          } catch {
            return await fail(IOError.read);
          }
          if (closed || result === stopped) return done();
          if (result.done) {
            if (!length) {
              await close();
              return done();
            }
            let value;
            try {
              value = decode(false);
            } catch {
              return await fail(IOError.utf8);
            }
            await close();
            return { done: false, value: ok(value) };
          }
          if (!(result.value instanceof Uint8Array))
            return await fail(IOError.read);
          chunk = result.value;
          offset = 0;
        }
        return done();
      } finally {
        reading = false;
      }
    },
    async return() {
      await close();
      return done();
    },
  };
}

export function createTextIO({
  input,
  write,
  maxLineBytes = 1024 * 1024,
  onEvent = () => {},
} = {}) {
  if (
    !Number.isInteger(maxLineBytes) ||
    maxLineBytes < 1 ||
    maxLineBytes > 8 * 1024 * 1024
  )
    throw new RangeError("maxLineBytes must be between 1 and 8388608");
  const io = {};
  if (input !== undefined)
    io.readLines = () => {
      if (input === undefined)
        return (async function* () {
          yield err(IOError.closed);
        })();
      const current = input;
      input = undefined;
      return lines(current, maxLineBytes, onEvent);
    };
  if (write !== undefined) {
    if (typeof write !== "function")
      throw new TypeError("expected output capability");
    io.writeLine = (text) => {
      if (typeof text !== "string") throw new TypeError("expected string");
      const bytes = new TextEncoder().encode(text + "\n");
      const controller = new AbortController();
      const operation = Promise.resolve().then(async () => {
        if (controller.signal.aborted) return err(IOError.closed);
        try {
          await write(bytes, controller.signal);
          return controller.signal.aborted
            ? err(IOError.closed)
            : ok(undefined);
        } catch {
          return err(
            controller.signal.aborted ? IOError.closed : IOError.write,
          );
        }
      });
      operation[Symbol.asyncDispose] = () => {
        controller.abort();
        return operation.then(() => {});
      };
      return operation;
    };
  }
  return io;
}
