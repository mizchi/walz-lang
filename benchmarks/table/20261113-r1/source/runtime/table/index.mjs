import { compile, op, desc, order } from "./expressions.mjs";
export { op, desc };
const token = Symbol("Walz table"), encoder = new TextEncoder(), decoder = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
const flatten = values => values.flatMap(value => Array.isArray(value) ? value : [value]);
function textValue(value) {
  if (typeof value !== "string" || !value.isWellFormed()) throw new TypeError("Table strings and column names must be well-formed Unicode");
  return value;
}
export async function instantiate(base, { filter = "simd" } = {}) {
  if (!["simd", "scalar", "packed-scalar"].includes(filter)) throw new TypeError("Unknown table filter execution mode");
  const url = new URL("core.wasm", base);
  const bytes = url.protocol === "file:" ? await (await import("node:fs/promises")).readFile(url) : await (await fetch(url)).arrayBuffer();
  const api = (await WebAssembly.instantiate(bytes)).instance.exports;
  const text = value => {
    const bytes = encoder.encode(textValue(value)), input = api["bytes-new"](bytes.length);
    bytes.forEach((value, i) => api["byte-set"](input, i, value)); return api.text(input);
  };
  const emptyText = text("");
  const fromText = value => {
    const bytes = new Uint8Array(api["text-len"](value));
    for (let i = 0; i < bytes.length; i++) bytes[i] = api["text-byte"](value, i);
    return decoder.decode(bytes);
  };
  const cell = value => {
    if (value === undefined) return api["cell-new"](0, 0, emptyText);
    if (value === null) return api["cell-new"](1, 0, emptyText);
    if (typeof value === "boolean") return api["cell-new"](2, Number(value), emptyText);
    if (typeof value === "number") return api["cell-new"](3, value, emptyText);
    if (typeof value === "string") return api["cell-new"](4, 0, text(value));
    throw new TypeError("Table cells support number, string, boolean, null, and undefined");
  };
  const fromCell = value => {
    switch (api["cell-kind"](value)) {
      case 0: return undefined; case 1: return null; case 2: return Boolean(api["cell-number"](value));
      case 3: return api["cell-number"](value); case 4: return fromText(api["cell-text"](value));
      default: throw new Error("Invalid Walz table cell");
    }
  };
  const array = (kind, values) => {
    const value = api[`${kind}s-new`](values.length);
    values.forEach((v, i) => api[`${kind}-set`](value, i, v)); return value;
  };
  const env = { api, text, fromText, cell, fromCell, array,
    filter: api[filter === "simd" ? "filter" : `filter-${filter}`] };
  function table(data) {
    if (!data || typeof data !== "object" || Array.isArray(data)) throw new TypeError("table expects named column arrays");
    const entries = Object.entries(data), columns = []; let length;
    for (const [name, values] of entries) {
      textValue(name);
      if (!Array.isArray(values) && !(ArrayBuffer.isView(values) && !(values instanceof DataView))) throw new TypeError(`Column ${name} must be an array or typed array`);
      if (values.length > 0x7fffffff) throw new RangeError("Table row count must fit nonnegative i32");
      if (length !== undefined && length !== values.length) throw new RangeError("All columns must have equal lengths");
      length = values.length;
      const input = api["cells-new"](length); let validType;
      for (let i = 0; i < length; i++) {
        const value = values[i];
        if (value != null) {
          const type = typeof value;
          if (validType !== undefined && validType !== type) throw new TypeError(`Column ${name} must use one non-null scalar type`);
          validType = type;
        }
        api["cell-set"](input, i, cell(value));
      }
      columns.push(api["column-new"](text(name), input));
    }
    return new Table(env, api["table-new"](array("column", columns), length ?? 0), token);
  }
  function from(rows) {
    if (!Array.isArray(rows)) return table(rows);
    const names = [...new Set(rows.flatMap(row => {
      if (!row || typeof row !== "object" || Array.isArray(row)) throw new TypeError("from expects object rows");
      return Object.keys(row);
    }))];
    if (rows.length > 0x7fffffff) throw new RangeError("Table row count must fit nonnegative i32");
    if (!names.length) return new Table(env, api["table-new"](array("column", []), rows.length), token);
    return table(Object.fromEntries(names.map(name => [name, rows.map(row => row[name])])));
  }
  return Object.freeze({ table, from, op, desc });
}

export class Table {
  #env; #ref; #names;
  constructor(env, ref, key) {
    if (key !== token) throw new TypeError("Use table() or from()");
    this.#env = env; this.#ref = ref;
    this.#names = Object.freeze(Array.from({ length: env.api["num-cols"](ref) }, (_, i) => env.fromText(env.api["column-name"](env.api.column(ref, i)))));
  }
  #new(ref) { return new Table(this.#env, ref, token); }
  #index(name) { const i = this.#names.indexOf(name); if (i < 0) throw new ReferenceError(`Unknown table column: ${name}`); return i; }
  #expression(value, options) { return compile(value, this.#names, this.#env, options); }
  numRows() { return this.#env.api["num-rows"](this.#ref); }
  numCols() { return this.#names.length; }
  columnNames() { return [...this.#names]; }
  isGrouped() { return Boolean(this.#env.api["is-grouped"](this.#ref)); }
  get(name, row = 0) {
    const index = this.#index(name);
    if (!Number.isInteger(row) || row < 0 || row >= this.numRows()) return undefined;
    return this.#env.fromCell(this.#env.api.get(this.#ref, index, row));
  }
  array(name) { this.#index(name); return Array.from({ length: this.numRows() }, (_, i) => this.get(name, i)); }
  objects() {
    return Array.from({ length: this.numRows() }, (_, row) => Object.fromEntries(this.#names.map(name => [name, this.get(name, row)])));
  }
  select(...names) {
    const { api, array } = this.#env;
    const selected = flatten(names).map(name => api.column(this.#ref, this.#index(name)));
    if (new Set(flatten(names)).size !== selected.length) throw new TypeError("Duplicate selected columns");
    return this.#new(api.select(this.#ref, array("column", selected)));
  }
  filter(predicate) { return this.#new(this.#env.filter(this.#ref, this.#expression(predicate))); }
  derive(specs) {
    const { api, array, text } = this.#env, names = this.columnNames(), columns = names.map((_, i) => api.column(this.#ref, i));
    const compiled = Object.entries(specs).map(([name, value]) => [textValue(name), this.#expression(value)]);
    for (const [name, expr] of compiled) {
      const value = api.derive(this.#ref, text(name), expr), index = names.indexOf(name);
      if (index < 0) { names.push(name); columns.push(value); } else columns[index] = value;
    }
    return this.#new(api.select(this.#ref, array("column", columns)));
  }
  groupby(...keys) {
    const { api, array, text } = this.#env;
    const columns = flatten(keys).flatMap(key => typeof key === "string" ? [api.column(this.#ref, this.#index(key))] :
      Object.entries(key).map(([name, value]) => api.derive(this.#ref, text(name), this.#expression(value, { column: true }))));
    return this.#new(api.groupby(this.#ref, array("column", columns)));
  }
  ungroup() { return this.#new(this.#env.api.ungroup(this.#ref)); }
  orderby(...keys) {
    const { api, array } = this.#env, specs = flatten(keys).map(order);
    return this.#new(api.orderby(this.#ref, array("expr", specs.map(({ value }) => this.#expression(value, { column: true }))), array("int", specs.map(s => s.descending))));
  }
  slice(start = 0, end = this.numRows()) {
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end)) throw new TypeError("Slice bounds must be safe integers");
    const n = this.numRows(), clamp = v => v < 0 ? Math.max(0, n + v) : Math.min(n, v);
    return this.#new(this.#env.api.slice(this.#ref, clamp(start), clamp(end)));
  }
  rollup(specs = {}) {
    const { api, array, text } = this.#env, entries = Object.entries(specs);
    return this.#new(api.rollup(this.#ref, array("name", entries.map(([name]) => text(name))), array("expr", entries.map(([, value]) => this.#expression(value, { aggregate: true })))));
  }
  count({ as = "count" } = {}) { return this.rollup({ [as]: op.count() }); }
  #join(other, on, mode, values, options = {}) {
    if (!(other instanceof Table) || other.#env !== this.#env) throw new TypeError("Joined tables must come from the same Walz instance");
    if (this.isGrouped() || other.isGrouped()) throw new TypeError("Ungroup tables before joining");
    if (values != null) throw new TypeError("Custom join projections are not supported; select or derive after joining");
    let left, right;
    if (on === undefined) {
      left = right = this.#names.filter(name => other.#names.includes(name));
    } else if (typeof on === "string") left = right = [on];
    else if (Array.isArray(on) && on.length === 1) left = right = [].concat(on[0]);
    else if (Array.isArray(on) && on.length === 2) { left = [].concat(on[0]); right = [].concat(on[1]); }
    else throw new TypeError("Use a column name or [leftKeys, rightKeys] for an equijoin");
    if (!left.length || left.length !== right.length) throw new TypeError("Join requires the same positive number of keys on both sides");
    const lkeys = left.map(name => this.#index(name)), rkeys = right.map(name => other.#index(name));
    const { api, array, text } = this.#env;
    const shared = new Set(left.filter((name, i) => name === right[i]));
    let specs;
    if (mode >= 4) specs = this.#names.map((name, i) => ({ name, side: 0, left: i, right: 0 }));
    else {
      const l = this.#names.flatMap((name, i) => mode === 2 && shared.has(name) ? [] : [{ name, side: mode === 3 && shared.has(name) ? 2 : 0, left: i, right: other.#names.indexOf(name) }]);
      const r = other.#names.flatMap((name, i) => mode !== 2 && shared.has(name) ? [] : [{ name, side: 1, left: 0, right: i }]);
      const collisions = new Set(l.filter(a => r.some(b => a.name === b.name)).map(a => a.name));
      for (const [side, fields] of [l, r].entries()) for (const spec of fields) if (collisions.has(spec.name)) spec.name += options.suffix?.[side] ?? `_${side + 1}`;
      specs = mode === 2 ? [...r, ...l] : [...l, ...r];
    }
    if (new Set(specs.map(s => s.name)).size !== specs.length) throw new TypeError("Join output column names collide");
    const layouts = specs.map(s => api["layout-new"](text(s.name), s.side, s.left, s.right));
    return this.#new(api.join(this.#ref, other.#ref, array("int", lkeys), array("int", rkeys), array("layout", layouts), mode));
  }
  join(other, on, values, options) { return this.#join(other, on, 0, values, options); }
  join_left(other, on, values, options) { return this.#join(other, on, 1, values, options); }
  join_right(other, on, values, options) { return this.#join(other, on, 2, values, options); }
  join_full(other, on, values, options) { return this.#join(other, on, 3, values, options); }
  semijoin(other, on) { return this.#join(other, on, 4); }
  antijoin(other, on) { return this.#join(other, on, 5); }
}
