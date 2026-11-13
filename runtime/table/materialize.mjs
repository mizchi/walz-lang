// Spread defines own data properties, including __proto__, without eval or
// inherited setters. The subsequent writes update those existing properties.
export function createRows(names, n) {
  const template = Object.fromEntries(names.map(name => [name, undefined]));
  const output = new Array(n);
  for (let row = 0; row < n; row++) {
    output[row] = { ...template };
  }
  return output;
}

export function materialize(names, columns, n) {
  const output = createRows(names, n);
  for (let row = 0; row < n; row++) for (let c = 0; c < names.length; c++) output[row][names[c]] = columns[c][row];
  return output;
}
