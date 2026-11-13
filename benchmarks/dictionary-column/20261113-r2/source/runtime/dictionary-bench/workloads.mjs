// Strong JS controls over the same validated string/i32 input domain as the
// guest. Histograms/postings are separate indexes with separately measured setup.
export function encode(values) {
  const dictionary = [], ids = new Map(), codes = new Int32Array(values.length);
  for (let i = 0; i < values.length; i++) {
    const value = values[i]; let id = ids.get(value);
    if (id === undefined) { id = dictionary.length; ids.set(value, id); dictionary.push(value); }
    codes[i] = id;
  }
  return { dictionary, codes };
}
export function validate(encoded) {
  const keys = new Set();
  for (const key of encoded.dictionary) { if (keys.has(key)) throw new RangeError("duplicate dictionary key"); keys.add(key); }
  for (const code of encoded.codes) if ((code >>> 0) >= keys.size) throw new RangeError("dictionary code");
  return { dictionary: encoded.dictionary, codes: encoded.codes.slice() };
}
export function histogram(values) {
  const counts = new Map();
  for (const key of values) counts.set(key, (counts.get(key) ?? 0) + 1);
  return counts;
}
export function countsRaw(values, queries) {
  const out = new Int32Array(queries.length);
  for (let q = 0; q < queries.length; q++) {
    const key = queries[q]; let count = 0;
    for (let i = 0; i < values.length; i++) count += Number(values[i] === key);
    out[q] = count;
  }
  return out;
}
export function countsEncoded(encoded, queries, ids) {
  const out = new Int32Array(queries.length), { dictionary, codes } = encoded;
  for (let q = 0; q < queries.length; q++) {
    const code = ids ? ids.get(queries[q]) : dictionary.indexOf(queries[q]);
    if (code === undefined || code < 0) continue;
    let count = 0;
    for (let i = 0; i < codes.length; i++) count += Number(codes[i] === code);
    out[q] = count;
  }
  return out;
}
export const countsHistogram = (index, queries) => Int32Array.from(queries, key => index.get(key) ?? 0);
export function countsValidated(encoded, queries) {
  const ids = new Map(), counts = new Int32Array(encoded.dictionary.length);
  encoded.dictionary.forEach((key, i) => { if (ids.has(key)) throw new RangeError("duplicate dictionary key"); ids.set(key, i); });
  for (const code of encoded.codes) {
    if ((code >>> 0) >= counts.length) throw new RangeError("dictionary code");
    counts[code]++;
  }
  return Int32Array.from(queries, key => { const code = ids.get(key); return code === undefined ? 0 : counts[code]; });
}
export function postings(encoded) {
  const { dictionary, codes } = encoded, counts = new Int32Array(dictionary.length);
  for (const code of codes) counts[code]++;
  const offsets = new Int32Array(dictionary.length + 1);
  for (let i = 0; i < counts.length; i++) offsets[i + 1] = offsets[i] + counts[i];
  const cursor = offsets.slice(), rows = new Int32Array(codes.length);
  for (let i = 0; i < codes.length; i++) rows[cursor[codes[i]]++] = i;
  return { ids: new Map(dictionary.map((key, i) => [key, i])), offsets, rows };
}
export function queryRaw(values, amounts, key, lo, hi) {
  const indices = []; let sum = 0;
  for (let i = 0; i < values.length; i++)
    if (values[i] === key && amounts[i] >= lo && amounts[i] < hi) { indices.push(i); sum += amounts[i]; }
  return { count: indices.length, sum, indices: Int32Array.from(indices) };
}
export function queryEncoded(encoded, amounts, key, lo, hi, branchless = false) {
  const code = encoded.dictionary.indexOf(key), { codes } = encoded;
  if (code < 0) return { count: 0, sum: 0, indices: new Int32Array() };
  if (branchless) {
    const out = new Int32Array(codes.length); let count = 0, sum = 0;
    for (let i = 0; i < codes.length; i++) {
      const hit = (codes[i] === code) & (amounts[i] >= lo) & (amounts[i] < hi);
      out[count] = i; count += hit; sum += amounts[i] * hit;
    }
    return { count, sum, indices: out.slice(0, count) };
  }
  const indices = []; let sum = 0;
  for (let i = 0; i < codes.length; i++)
    if (codes[i] === code && amounts[i] >= lo && amounts[i] < hi) { indices.push(i); sum += amounts[i]; }
  return { count: indices.length, sum, indices: Int32Array.from(indices) };
}
export function queryPostings(index, amounts, key, lo, hi) {
  const id = index.ids.get(key), indices = []; let sum = 0;
  if (id !== undefined) for (let p = index.offsets[id]; p < index.offsets[id + 1]; p++) {
    const i = index.rows[p], value = amounts[i];
    if (value >= lo && value < hi) { indices.push(i); sum += value; }
  }
  return { count: indices.length, sum, indices: Int32Array.from(indices) };
}
export function dataset(n, cardinality, unicode = false) {
  if (!Number.isInteger(n) || n < 1 || !Number.isInteger(cardinality) || cardinality < 1 || cardinality > n)
    throw new RangeError("positive n >= cardinality required");
  const label = id => `${unicode ? "地域🦀-" : "category-"}${id}`;
  const vocabulary = Array.from({ length: cardinality + 1 }, (_, i) => label(i));
  let seed = (137 ^ n ^ cardinality) | 0;
  const next = () => seed = (Math.imul(seed, 1664525) + 1013904223) | 0;
  // One initial occurrence of every key fixes actual cardinality. Later IDs
  // use high RNG bits rather than the correlated low LCG bits.
  const ids = Int32Array.from({ length: n }, (_, i) => i < cardinality ? i : (next() >>> 8) % cardinality);
  const values = Array.from(ids, id => vocabulary[id]);
  const amounts = Int32Array.from({ length: n }, () => (next() >>> 8) % 8192 - 4096);
  amounts[0] = 42; // A qualifying row is a positive control for full result checks.
  const queryIds = Int32Array.from({ length: 32 }, (_, i) => i % 8 === 7 ? cardinality : (i * 31) % cardinality);
  const queries = Array.from(queryIds, id => vocabulary[id]);
  return { n, cardinality, unicode, vocabulary, ids, values, amounts, queries, queryIds, lo: -500, hi: 500 };
}
