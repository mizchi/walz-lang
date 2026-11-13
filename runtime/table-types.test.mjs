import test from "node:test";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { resolve, join } from "node:path";

test("table declarations infer scalar fields and reject invalid schemas and constructors", async () => {
  const directory = await mkdtemp(resolve(".table-types-"));
  try {
    const file = join(directory, "usage.ts");
    await writeFile(file, `import {instantiate, op, desc, Table} from '@mizchi/walz-runtime/table';
const aq = await instantiate(new URL('file:///tmp/table/'));
await instantiate(new URL('file:///tmp/table/'), {filter: 'packed-scalar'});
await instantiate(new URL('file:///tmp/table/'), {io: 'scalar'});
// @ts-expect-error unknown transfer mode
await instantiate(new URL('file:///tmp/table/'), {io: 'invalid'});
// @ts-expect-error unknown execution mode
await instantiate(new URL('file:///tmp/table/'), {filter: 'invalid'});
const table = aq.table({country: ['JP', 'US'], amount: [1, 2]});
const selected = table.filter(d => d.amount > 0).derive({total: d => d.amount * 2});
const result = selected.groupby('country').rollup({sum: op.sum('total')}).orderby(desc('sum'));
const n: number | undefined = result.get('sum', 0);
const countries: string[] = table.array('country');
const doubles: number[] = selected.array('total');
const inline = table.rollup({sum: d => op.sum(d.amount)});
const inlineSum: number | undefined = inline.get('sum');
// @ts-expect-error unknown column
table.select('missing');
// @ts-expect-error unknown row field
table.filter(d => d.missing > 0);
// @ts-expect-error object cells are not scalar values
aq.table({bad: [{x: 1}]});
// @ts-expect-error direct construction is not public
new Table();
// @ts-expect-error aggregate descriptors are not row predicates
table.filter(op.count());
// @ts-expect-error an empty selection has no fields
table.select().get('amount');
// @ts-expect-error nested aggregate descriptors are not supported
op.sum(op.mean('amount'));
// @ts-expect-error mixed scalar types in one input column
aq.table({bad: [1, 'x']});
void [n, countries, doubles, inlineSum];
`);
    await promisify(execFile)(resolve("node_modules/.bin/tsc"), ["--noEmit", "--strict", "--target", "esnext", "--module", "nodenext", file], { maxBuffer: 4 * 1024 * 1024 });
  } finally { await rm(directory, { recursive: true, force: true }); }
});
