// Firefox processed-profile format produced by Wasmtime GuestProfiler.
export function summarize(profile) {
  const self = new Map(),
    inclusive = new Map();
  let samples = 0;
  const add = (map, key, weight) => map.set(key, (map.get(key) ?? 0) + weight);
  for (const thread of profile.threads) {
    const {
      samples: rows,
      stackTable: stacks,
      frameTable: frames,
      funcTable: funcs,
      resourceTable: resources,
      stringArray: strings,
    } = thread;
    for (let i = 0; i < rows.length; i++) {
      let stack = rows.stack[i];
      if (stack === null) continue;
      const weight = rows.weight?.[i] ?? 1;
      samples += weight;
      const seen = new Set();
      let leaf = true;
      while (stack !== null) {
        const fn = frames.func[stacks.frame[stack]],
          resource = funcs.resource[fn];
        const module =
          resource !== null && resource >= 0
            ? strings[resources.name[resource]]
            : "";
        const label = `${module}::${strings[funcs.name[fn]]}`;
        if (leaf) {
          add(self, label, weight);
          leaf = false;
        }
        if (!seen.has(label)) {
          add(inclusive, label, weight);
          seen.add(label);
        }
        stack = stacks.prefix[stack];
      }
    }
  }
  const rank = (map) =>
    [...map]
      .sort((a, b) => b[1] - a[1])
      .map(([name, count]) => ({
        name,
        samples: count,
        percent: (100 * count) / samples,
      }));
  return {
    samples,
    intervalMs: profile.meta.interval,
    self: rank(self),
    inclusive: rank(inclusive),
  };
}
