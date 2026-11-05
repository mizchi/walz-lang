// Independent specification for the example. This model uses JS Sets/Maps and
// completion timestamps; the guest uses GC arrays and an enum of job states.
export function workflowOracle(lines) {
  const jobs = new Map(),
    records = [];
  let edges = 0;
  const validId = (s) => s && Buffer.byteLength(s) <= 128;
  for (const [index, line] of lines.entries()) {
    const lineNumber = index + 1;
    const fail = (error, detail) =>
      records.push({ line: lineNumber, ok: false, error, detail });
    if (!line || line.startsWith("#")) continue;
    const fields = line.split("\t"),
      [op, a, b] = fields;
    const number = (s) => (/^\d+$/.test(s ?? "") ? Number(s) : NaN);
    if (
      op === "TASK" &&
      fields.length === 3 &&
      validId(a) &&
      number(b) <= 1_000_000
    ) {
      if (jobs.has(a)) fail("duplicate-task", a);
      else if (jobs.size === 4096) fail("capacity", "tasks");
      else jobs.set(a, { duration: number(b), dependencies: new Set() });
    } else if (
      op === "AFTER" &&
      fields.length === 3 &&
      validId(a) &&
      validId(b)
    ) {
      if (!jobs.has(a)) fail("unknown-task", a);
      else if (!jobs.has(b)) fail("unknown-task", b);
      else if (!jobs.get(a).dependencies.has(b)) {
        if (edges === 65536) fail("capacity", "dependencies");
        else {
          jobs.get(a).dependencies.add(b);
          edges++;
        }
      }
    } else if (
      op === "PLAN" &&
      fields.length === 2 &&
      number(a) >= 1 &&
      number(a) <= 32
    ) {
      const workers = number(a),
        pending = new Set(jobs.keys()),
        done = new Set(),
        running = new Map(),
        result = new Map();
      let now = 0;
      while (pending.size || running.size) {
        for (const [id, task] of running)
          if (task.finish === now) {
            done.add(id);
            running.delete(id);
          }
        for (let worker = 0; worker < workers; worker++) {
          if ([...running.values()].some((task) => task.worker === worker))
            continue;
          const id = [...pending].find((id) =>
            [...jobs.get(id).dependencies].every((dep) => done.has(dep)),
          );
          if (id === undefined) continue;
          pending.delete(id);
          const task = {
            worker,
            start: now,
            finish: now + jobs.get(id).duration,
          };
          running.set(id, task);
          result.set(id, task);
        }
        if (running.size)
          now = Math.min(...[...running.values()].map((task) => task.finish));
        else if (pending.size) break;
      }
      if (pending.size) fail("cycle", pending.values().next().value);
      else {
        records.push({
          line: lineNumber,
          ok: true,
          kind: "plan",
          workers,
          tasks: jobs.size,
          makespan: now,
        });
        for (const id of jobs.keys())
          records.push({
            line: lineNumber,
            kind: "job",
            id,
            ...result.get(id),
          });
      }
    } else fail("invalid-command", "format");
  }
  return records;
}
