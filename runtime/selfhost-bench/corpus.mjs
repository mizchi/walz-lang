// v2 keeps v1 source/probes unchanged and adds the Rust WAT+validator reference.
export const suite = "walz-selfhost-parity-v2";
const encode = n => String(n);
const i32 = n => n | 0;

function expressions(count) {
  let seed = 0x5e1f2026;
  const random = () => {
    seed ^= seed << 13; seed ^= seed >>> 17; seed ^= seed << 5;
    return seed >>> 0;
  };
  const operators = [
    ["i32.add", (a, b) => i32(a + b)],
    ["i32.sub", (a, b) => i32(a - b)],
    ["i32.mul", Math.imul],
    ["i32.xor", (a, b) => a ^ b],
    ["i32.shl", (a, b) => a << b],
    ["i32.shr_u", (a, b) => i32(a >>> b)],
  ];
  function expression(depth) {
    if (depth === 0) {
      if (random() & 1) return { text: "(local.get $x)", run: x => x };
      const value = random() | 0;
      return { text: `(i32.const ${value})`, run: () => value };
    }
    const [name, apply] = operators[random() % operators.length];
    const a = expression(depth - 1), b = expression(depth - 1);
    return { text: `(${name} ${a.text} ${b.text})`, run: x => apply(a.run(x), b.run(x)) };
  }
  return Array.from({ length: count }, () => expression(3));
}

function generated(count) {
  const expressions_ = expressions(count);
  return {
    id: `expressions-${count}`,
    source: `(module\n${expressions_.map((e, i) => `  (func $f${i} (export "f${i}") (param $x i32) (result i32) ${e.text})`).join("\n")}\n)\n`,
    probes: expressions_.flatMap((e, i) => [0, 1, -1, 0x7fffffff, -0x80000000, 0x5e1f2026].map(x => ({
      name: `f${i}`, args: [x], expected: e.run(x),
    }))),
  };
}

export function corpus() {
  const tiny = generated(1);
  tiny.id = "integer-mix";
  const wide = Array.from({ length: 192 }, (_, i) =>
    `(func $f${i} (result i32) ${i === 191 ? "(i32.const 42)" : `(call $f${i + 1})`})`).join("\n");
  return [tiny, {
    id: "i64-loop",
    source: `(module
  (func $sum (export "sum") (param $n i64) (result i64) (local $acc i64)
    (block (result i64)
      (block $done (loop $again
        (br_if $done (i64.eqz (local.get $n)))
        (local.set $acc (i64.add (local.get $acc) (local.get $n)))
        (local.set $n (i64.sub (local.get $n) (i64.const 1)))
        (br $again)))
      (local.get $acc))))\n`,
    probes: [0n, 1n, 100n, 1000n].map(n => ({ name: "sum", args: [n], expected: n * (n + 1n) / 2n })),
  }, {
    id: "recursive-call",
    source: `(module
  (func $fac (export "fac") (param $n i32) (result i32)
    (if (result i32) (i32.le_s (local.get $n) (i32.const 1))
      (then (i32.const 1))
      (else (i32.mul (local.get $n) (call $fac (i32.sub (local.get $n) (i32.const 1))))))))\n`,
    probes: [0, 1, 2, 6, 10, 13].map(n => {
      let result = 1; for (let i = 2; i <= n; i++) result = Math.imul(result, i);
      return { name: "fac", args: [n], expected: result };
    }),
  }, {
    id: "branches-and-traps",
    source: `(module
  (func $safe (export "safe") (param $x i32) (result i32)
    (if (result i32) (i32.eqz (local.get $x))
      (then (i32.const 42)) (else (i32.div_s (i32.const 120) (local.get $x)))))
  (func $divide (export "divide") (param $a i32) (param $b i32) (result i32)
    (i32.div_s (local.get $a) (local.get $b))))\n`,
    probes: [
      ...[0, 1, -1, 5].map(x => ({ name: "safe", args: [x], expected: x === 0 ? 42 : i32(120 / x) })),
      { name: "divide", args: [42, 2], expected: 21 },
      { name: "divide", args: [1, 0], trap: true },
      { name: "divide", args: [-2147483648, -1], trap: true },
    ],
  }, {
    id: "i64-bits",
    source: `(module
  (func $mix (export "mix") (param $x i64) (result i64)
    (i64.xor (i64.shl (local.get $x) (i64.const 7)) (i64.shr_u (local.get $x) (i64.const 9))))
  (func $extend (export "extend") (param $x i32) (result i64) (i64.extend_i32_s (local.get $x)))
  (func $wrap (export "wrap") (param $x i64) (result i32) (i32.wrap_i64 (local.get $x))))\n`,
    probes: [
      ...[0n, 1n, -1n, -(1n << 63n), (1n << 63n) - 1n].map(x => ({ name: "mix", args: [x],
        expected: BigInt.asIntN(64, (x << 7n) ^ (BigInt.asUintN(64, x) >> 9n)) })),
      { name: "extend", args: [-2147483648], expected: -2147483648n },
      { name: "wrap", args: [0x10000002an], expected: 42 },
    ],
  }, {
    id: "wide-indices",
    source: `(module\n${wide}\n(func $main (export "main") (result i32) (call $f0))
      (func $last (export "last") ${Array.from({length: 130}, (_, i) => `(param $p${i} i32)`).join(" ")} (result i32) (local.get $p129)))\n`,
    probes: [{ name: "main", args: [], expected: 42 }, { name: "last", args: Array.from({length: 130}, (_, i) => i), expected: 129 }],
  }, generated(32), generated(128)];
}

export function serializable(value) {
  return JSON.stringify(value, (_, v) => typeof v === "bigint" ? { i64: encode(v) } : v, 2) + "\n";
}
