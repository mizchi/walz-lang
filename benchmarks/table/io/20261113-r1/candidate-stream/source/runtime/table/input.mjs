// Take the input snapshot before borrowing Wasm scratch: array getters can
// reenter the facade. Local typed arrays avoid boxed numeric staging values.
export function readChunk(values, start, n, validType, name) {
  const kinds = new Uint8Array(n), numbers = new Float64Array(n); let strings;
  for (let i = 0; i < n; i++) {
    const value = values[start + i], type = typeof value;
    if (value != null) {
      if (type !== "number" && type !== "boolean" && type !== "string") throw new TypeError("Table cells support number, string, boolean, null, and undefined");
      if (validType !== undefined && validType !== type) throw new TypeError(`Column ${name} must use one non-null scalar type`);
      validType = type;
    }
    if (type === "string") {
      if (!value.isWellFormed()) throw new TypeError("Table strings and column names must be well-formed Unicode");
      strings ??= new Array(n); strings[i] = value; kinds[i] = 4;
    } else {
      kinds[i] = value === undefined ? 0 : value === null ? 1 : type === "boolean" ? 2 : 3;
      numbers[i] = value == null ? 0 : Number(value);
    }
  }
  return { kinds, numbers, strings, validType };
}
