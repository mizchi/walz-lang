// Spread defines own data properties, including __proto__, without eval or
// inherited setters. The subsequent writes update those existing properties.
export function materialize(names, columns, n) {
  const template = Object.fromEntries(names.map(name => [name, undefined]));
  const output = new Array(n);
  for (let row = 0; row < n; row++) {
    const value = { ...template };
    for (let c = 0; c < names.length; c++) value[names[c]] = columns[c][row];
    output[row] = value;
  }
  return output;
}
