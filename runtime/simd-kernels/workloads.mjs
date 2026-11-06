// Independent scalar references. Integer sums stay exact below 2^53.
export function sum(xs) {
  let total = 0;
  for (let i = 0; i < xs.length; i++) total += xs[i];
  return total;
}
export function dot(a, b) {
  let total = 0;
  for (let i = 0; i < a.length; i++) total += a[i] * b[i];
  return total;
}
export function intersection(a, b) {
  let total = 0;
  for (let i = 0; i < a.length; i++) {
    let x = a[i] & b[i];
    x -= (x >>> 1) & 0x55555555;
    x = (x & 0x33333333) + ((x >>> 2) & 0x33333333);
    x = (x + (x >>> 4)) & 0x0f0f0f0f;
    total += (Math.imul(x, 0x01010101) >>> 24);
  }
  return total;
}
export function corpus(n) {
  let seed = (20261106 ^ n) | 0;
  const next = () => (seed = (Math.imul(seed, 1664525) + 1013904223) | 0);
  const a = Int32Array.from({ length: n }, next);
  const b = Int32Array.from({ length: n }, next);
  // Bounded integer-valued floats keep these benchmark reductions exact in f32.
  // Fractional/rounding correctness is checked separately by the runtime tests.
  const fa = Float32Array.from({ length: n }, () => (next() >>> 29) - 3);
  const fb = Float32Array.from({ length: n }, () => (next() >>> 29) - 3);
  const bytes = Uint8Array.from({ length: n }, () => (next() >>> 0) % 251);
  const late = bytes.slice(); if (n) late[n - 1] = 255;
  const early = bytes.slice(); if (n) early[0] = 255;
  return [
    { name: "i32-sum", kind: "i32", a, expected: sum(a), scalar: "sum-scalar", simd: "sum-simd", portable: "sum", js: () => sum(a) },
    { name: "f32-dot", kind: "f32", a: fa, b: fb, expected: dot(fa, fb), scalar: "dot-scalar", simd: "dot-simd", portable: "dot", js: () => dot(fa, fb) },
    { name: "bitmap-intersection", kind: "i32", a, b, expected: intersection(a, b), scalar: "bitmap-scalar", simd: "bitmap-simd", portable: "bitmap", js: () => intersection(a, b) },
    ...[["miss", bytes], ["late", late], ["early", early]].map(([name, a]) => ({
      name: `bytes-index-${name}`, kind: "bytes", a, needle: 255,
      expected: a.indexOf(255), scalar: "bytes-scalar", simd: "bytes-simd", portable: "index", js: () => a.indexOf(255),
    })),
  ];
}
