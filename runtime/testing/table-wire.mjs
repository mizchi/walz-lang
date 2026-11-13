// Test/example value adapters. Every table operation still executes in Walz.
import { compile } from "../table/expressions.mjs";
export function cell(value) {
  const kind = value === undefined ? 0 : value === null ? 1 : typeof value === "boolean" ? 2 : typeof value === "number" ? 3 : typeof value === "string" ? 4 : -1;
  if (kind < 0 || kind === 4 && !value.isWellFormed()) throw new TypeError("Unsupported table cell");
  return { kind, number: kind === 2 || kind === 3 ? Number(value) : 0, text: kind === 4 ? value : "" };
}
export function snapshot(data) {
  const columns = Object.entries(data).map(([name, values]) => ({ name, values: Array.from(values, cell) }));
  const length = columns[0]?.values.length ?? 0;
  if (columns.some(c => c.values.length !== length)) throw new RangeError("Column lengths differ");
  return { columns, length };
}
export function objects(snapshot) {
  const decode = value => value.kind === 0 ? undefined : value.kind === 1 ? null : value.kind === 2 ? Boolean(value.number) : value.kind === 3 ? value.number : value.text;
  return Array.from({ length: snapshot.length }, (_, i) => Object.fromEntries(snapshot.columns.map(c => [c.name, decode(c.values[i])])));
}
export function program(expression, names, options) {
  const nodes = [], value = cell(undefined);
  const append = node => (nodes.push(node), nodes.length - 1);
  const api = {
    literal: value => append({ op: 0, value, a: -1, b: -1, c: -1 }),
    field: a => append({ op: 1, value, a, b: -1, c: -1 }),
    node: (op, a, b, c) => append({ op, value, a, b, c }),
  };
  return { nodes, root: compile(expression, names, { api, cell }, options) };
}
export const command = (op, values = {}) => ({ op, names: [], programs: [], indices: [], other: { columns: [], length: 0 }, leftKeys: [], rightKeys: [], layout: [], mode: 0, ...values });
