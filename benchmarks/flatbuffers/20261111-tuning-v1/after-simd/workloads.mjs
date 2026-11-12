import { createHash } from "node:crypto";
export const hash = bytes => createHash("sha256").update(bytes).digest("hex");
export function materialize(value) {
  return { ...value, id: BigInt(value.id), values: Int32Array.from(value.values), payload: Uint8Array.from(value.payload), child: value.child };
}
export function corpus() {
  const base = { id: "9007199254740993", name: "message", values: [20, -1, 22], payload: [0, 128, 255], enabled: false, weight: -1.25 };
  const inputs = {
    small: { ...base },
    "unicode-4k": { ...base, name: "日本語🦀\0".repeat(16), values: Array.from({ length: 64 }, (_, i) => i - 32), payload: Array.from({ length: 4096 }, (_, i) => i & 255), child: { count: 42, label: "nested-日本語" } },
    "bytes-64k": { ...base, payload: Array.from({ length: 65536 }, (_, i) => (i * 31 + 7) & 255), child: { count: -2147483648, label: "payload" } },
    "values-16k": { ...base, values: Array.from({ length: 16384 }, (_, i) => Math.imul(i - 8192, 65537)), payload: Array.from({ length: 256 }, (_, i) => i) },
  };
  return Object.entries(inputs).map(([name, value]) => ({ name, value, inputHash: hash(JSON.stringify(value)) }));
}
