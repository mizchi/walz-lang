// Handwritten schema adapter using the official FlatBuffers JS library.
// No deduplication-only test objects or intermediate JS number arrays.
export function createReference({ Builder, ByteBuffer }) {
  function encodeWith(b, m) {
    const name = b.createString(m.name);
    b.startVector(4, m.values.length, 4);
    for (let i = m.values.length - 1; i >= 0; i--) b.addInt32(m.values[i]);
    const values = b.endVector(), payload = b.createByteVector(m.payload);
    let child = 0;
    if (m.child) {
      const label = b.createString(m.child.label);
      b.startObject(2);
      b.addFieldInt32(0, m.child.count, 0);
      b.addFieldOffset(1, label, 0);
      child = b.endObject();
    }
    b.startObject(7);
    b.addFieldInt64(0, m.id, 0n);
    b.addFieldOffset(1, name, 0);
    b.addFieldOffset(2, values, 0);
    b.addFieldOffset(3, payload, 0);
    b.addFieldInt8(4, Number(m.enabled), 1);
    b.addFieldFloat64(5, m.weight, 0);
    if (child) b.addFieldOffset(6, child, 0);
    b.finish(b.endObject(), "WLZF");
    // Like Walz's immutable bytes, the returned storage survives builder reuse.
    return b.asUint8Array().slice();
  }
  const pointer = (bb, t, slot) => {
    const offset = bb.__offset(t, 4 + slot * 2);
    return offset ? t + offset : 0;
  };
  const text = (bb, t, slot) => {
    const p = pointer(bb, t, slot);
    return p ? bb.__string(p) : "";
  };
  return {
    encode: m => encodeWith(new Builder(64), m),
    createEncoder() {
      const builder = new Builder(64);
      return m => { builder.clear(); return encodeWith(builder, m); };
    },
    decode(bytes) {
      const bb = new ByteBuffer(bytes);
      if (bb.getBufferIdentifier() !== "WLZF") throw new Error("wrong FlatBuffers identifier");
      const t = bb.readUint32(0), p0 = pointer(bb, t, 0), p2 = pointer(bb, t, 2);
      const p3 = pointer(bb, t, 3), p4 = pointer(bb, t, 4), p5 = pointer(bb, t, 5), p6 = pointer(bb, t, 6);
      const child = p6 ? bb.__indirect(p6) : 0;
      // Equivalent to generated *Array accessors followed by an owning copy.
      // Benchmark inputs use aligned buffers on a little-endian machine.
      return {
        id: p0 ? bb.readInt64(p0) : 0n, name: text(bb, t, 1),
        values: p2 ? new Int32Array(bytes.buffer, bytes.byteOffset + bb.__vector(p2), bb.__vector_len(p2)).slice() : new Int32Array(),
        payload: p3 ? bytes.slice(bb.__vector(p3), bb.__vector(p3) + bb.__vector_len(p3)) : new Uint8Array(),
        enabled: p4 ? Boolean(bb.readUint8(p4)) : true,
        weight: p5 ? bb.readFloat64(p5) : 0,
        child: child ? { count: pointer(bb, child, 0) ? bb.readInt32(pointer(bb, child, 0)) : 0, label: text(bb, child, 1) } : undefined,
      };
    },
  };
}
