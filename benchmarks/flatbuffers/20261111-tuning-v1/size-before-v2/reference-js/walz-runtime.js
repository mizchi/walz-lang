// \0rolldown/runtime.js
var e = (e, t) => () => (t || (e((t = { exports: {} }).exports, t, t.exports), e = null), t.exports), t = /* @__PURE__ */ e(((e) => {
	Object.defineProperty(e, "__esModule", { value: !0 }), e.SIZE_PREFIX_LENGTH = e.FILE_IDENTIFIER_LENGTH = e.SIZEOF_INT = e.SIZEOF_SHORT = void 0, e.SIZEOF_SHORT = 2, e.SIZEOF_INT = 4, e.FILE_IDENTIFIER_LENGTH = 4, e.SIZE_PREFIX_LENGTH = 4;
})), n = /* @__PURE__ */ e(((e) => {
	Object.defineProperty(e, "__esModule", { value: !0 }), e.isLittleEndian = e.float64 = e.float32 = e.int32 = void 0, e.int32 = /* @__PURE__ */ new Int32Array(2), e.float32 = new Float32Array(e.int32.buffer), e.float64 = new Float64Array(e.int32.buffer), e.isLittleEndian = new Uint16Array(new Uint8Array([1, 0]).buffer)[0] === 1;
})), r = /* @__PURE__ */ e(((e) => {
	Object.defineProperty(e, "__esModule", { value: !0 }), e.Encoding = void 0;
	var t;
	(function(e) {
		e[e.UTF8_BYTES = 1] = "UTF8_BYTES", e[e.UTF16_STRING = 2] = "UTF16_STRING";
	})(t || (e.Encoding = t = {}));
})), i = /* @__PURE__ */ e(((e) => {
	Object.defineProperty(e, "__esModule", { value: !0 }), e.ByteBuffer = void 0;
	var i = t(), a = r(), o = n();
	e.ByteBuffer = class e {
		constructor(e) {
			this.bytes_ = e, this.position_ = 0, this.text_decoder_ = new TextDecoder();
		}
		static allocate(t) {
			return new e(new Uint8Array(t));
		}
		clear() {
			this.position_ = 0;
		}
		bytes() {
			return this.bytes_;
		}
		position() {
			return this.position_;
		}
		setPosition(e) {
			this.position_ = e;
		}
		capacity() {
			return this.bytes_.length;
		}
		readInt8(e) {
			return this.readUint8(e) << 24 >> 24;
		}
		readUint8(e) {
			return this.bytes_[e];
		}
		readInt16(e) {
			return this.readUint16(e) << 16 >> 16;
		}
		readUint16(e) {
			return this.bytes_[e] | this.bytes_[e + 1] << 8;
		}
		readInt32(e) {
			return this.bytes_[e] | this.bytes_[e + 1] << 8 | this.bytes_[e + 2] << 16 | this.bytes_[e + 3] << 24;
		}
		readUint32(e) {
			return this.readInt32(e) >>> 0;
		}
		readInt64(e) {
			return BigInt.asIntN(64, BigInt(this.readUint32(e)) + (BigInt(this.readUint32(e + 4)) << BigInt(32)));
		}
		readUint64(e) {
			return BigInt.asUintN(64, BigInt(this.readUint32(e)) + (BigInt(this.readUint32(e + 4)) << BigInt(32)));
		}
		readFloat32(e) {
			return o.int32[0] = this.readInt32(e), o.float32[0];
		}
		readFloat64(e) {
			return o.int32[+!o.isLittleEndian] = this.readInt32(e), o.int32[+!!o.isLittleEndian] = this.readInt32(e + 4), o.float64[0];
		}
		writeInt8(e, t) {
			this.bytes_[e] = t;
		}
		writeUint8(e, t) {
			this.bytes_[e] = t;
		}
		writeInt16(e, t) {
			this.bytes_[e] = t, this.bytes_[e + 1] = t >> 8;
		}
		writeUint16(e, t) {
			this.bytes_[e] = t, this.bytes_[e + 1] = t >> 8;
		}
		writeInt32(e, t) {
			this.bytes_[e] = t, this.bytes_[e + 1] = t >> 8, this.bytes_[e + 2] = t >> 16, this.bytes_[e + 3] = t >> 24;
		}
		writeUint32(e, t) {
			this.bytes_[e] = t, this.bytes_[e + 1] = t >> 8, this.bytes_[e + 2] = t >> 16, this.bytes_[e + 3] = t >> 24;
		}
		writeInt64(e, t) {
			this.writeInt32(e, Number(BigInt.asIntN(32, t))), this.writeInt32(e + 4, Number(BigInt.asIntN(32, t >> BigInt(32))));
		}
		writeUint64(e, t) {
			this.writeUint32(e, Number(BigInt.asUintN(32, t))), this.writeUint32(e + 4, Number(BigInt.asUintN(32, t >> BigInt(32))));
		}
		writeFloat32(e, t) {
			o.float32[0] = t, this.writeInt32(e, o.int32[0]);
		}
		writeFloat64(e, t) {
			o.float64[0] = t, this.writeInt32(e, o.int32[+!o.isLittleEndian]), this.writeInt32(e + 4, o.int32[+!!o.isLittleEndian]);
		}
		getBufferIdentifier() {
			if (this.bytes_.length < this.position_ + i.SIZEOF_INT + i.FILE_IDENTIFIER_LENGTH) throw Error("FlatBuffers: ByteBuffer is too short to contain an identifier.");
			let e = "";
			for (let t = 0; t < i.FILE_IDENTIFIER_LENGTH; t++) e += String.fromCharCode(this.readInt8(this.position_ + i.SIZEOF_INT + t));
			return e;
		}
		__offset(e, t) {
			let n = e - this.readInt32(e);
			return t < this.readInt16(n) ? this.readInt16(n + t) : 0;
		}
		__union(e, t) {
			return e.bb_pos = t + this.readInt32(t), e.bb = this, e;
		}
		__string(e, t) {
			e += this.readInt32(e);
			let n = this.readInt32(e);
			e += i.SIZEOF_INT;
			let r = this.bytes_.subarray(e, e + n);
			return t === a.Encoding.UTF8_BYTES ? r : this.text_decoder_.decode(r);
		}
		__union_with_string(e, t) {
			return typeof e == "string" ? this.__string(t) : this.__union(e, t);
		}
		__indirect(e) {
			return e + this.readInt32(e);
		}
		__vector(e) {
			return e + this.readInt32(e) + i.SIZEOF_INT;
		}
		__vector_len(e) {
			return this.readInt32(e + this.readInt32(e));
		}
		__has_identifier(e) {
			if (e.length != i.FILE_IDENTIFIER_LENGTH) throw Error("FlatBuffers: file identifier must be length " + i.FILE_IDENTIFIER_LENGTH);
			for (let t = 0; t < i.FILE_IDENTIFIER_LENGTH; t++) if (e.charCodeAt(t) != this.readInt8(this.position() + i.SIZEOF_INT + t)) return !1;
			return !0;
		}
		createScalarList(e, t) {
			let n = [];
			for (let r = 0; r < t; ++r) {
				let t = e(r);
				t !== null && n.push(t);
			}
			return n;
		}
		createObjList(e, t) {
			let n = [];
			for (let r = 0; r < t; ++r) {
				let t = e(r);
				t !== null && n.push(t.unpack());
			}
			return n;
		}
	};
})), a = /* @__PURE__ */ e(((e) => {
	Object.defineProperty(e, "__esModule", { value: !0 }), e.Builder = void 0;
	var n = i(), r = t();
	e.Builder = class e {
		constructor(e) {
			this.minalign = 1, this.vtable = null, this.vtable_in_use = 0, this.isNested = !1, this.object_start = 0, this.vtables = [], this.vector_num_elems = 0, this.force_defaults = !1, this.string_maps = null, this.text_encoder = new TextEncoder();
			let t;
			t = e || 1024, this.bb = n.ByteBuffer.allocate(t), this.space = t;
		}
		clear() {
			this.bb.clear(), this.space = this.bb.capacity(), this.minalign = 1, this.vtable = null, this.vtable_in_use = 0, this.isNested = !1, this.object_start = 0, this.vtables = [], this.vector_num_elems = 0, this.force_defaults = !1, this.string_maps = null;
		}
		forceDefaults(e) {
			this.force_defaults = e;
		}
		dataBuffer() {
			return this.bb;
		}
		asUint8Array() {
			return this.bb.bytes().subarray(this.bb.position(), this.bb.position() + this.offset());
		}
		prep(t, n) {
			t > this.minalign && (this.minalign = t);
			let r = ~(this.bb.capacity() - this.space + n) + 1 & t - 1;
			for (; this.space < r + t + n;) {
				let t = this.bb.capacity();
				this.bb = e.growByteBuffer(this.bb), this.space += this.bb.capacity() - t;
			}
			this.pad(r);
		}
		pad(e) {
			for (let t = 0; t < e; t++) this.bb.writeInt8(--this.space, 0);
		}
		writeInt8(e) {
			this.bb.writeInt8(--this.space, e);
		}
		writeInt16(e) {
			this.bb.writeInt16(this.space -= 2, e);
		}
		writeInt32(e) {
			this.bb.writeInt32(this.space -= 4, e);
		}
		writeInt64(e) {
			this.bb.writeInt64(this.space -= 8, e);
		}
		writeFloat32(e) {
			this.bb.writeFloat32(this.space -= 4, e);
		}
		writeFloat64(e) {
			this.bb.writeFloat64(this.space -= 8, e);
		}
		addInt8(e) {
			this.prep(1, 0), this.writeInt8(e);
		}
		addInt16(e) {
			this.prep(2, 0), this.writeInt16(e);
		}
		addInt32(e) {
			this.prep(4, 0), this.writeInt32(e);
		}
		addInt64(e) {
			this.prep(8, 0), this.writeInt64(e);
		}
		addFloat32(e) {
			this.prep(4, 0), this.writeFloat32(e);
		}
		addFloat64(e) {
			this.prep(8, 0), this.writeFloat64(e);
		}
		addFieldInt8(e, t, n) {
			(this.force_defaults || t != n) && (this.addInt8(t), this.slot(e));
		}
		addFieldInt16(e, t, n) {
			(this.force_defaults || t != n) && (this.addInt16(t), this.slot(e));
		}
		addFieldInt32(e, t, n) {
			(this.force_defaults || t != n) && (this.addInt32(t), this.slot(e));
		}
		addFieldInt64(e, t, n) {
			(this.force_defaults || t !== n) && (this.addInt64(t), this.slot(e));
		}
		addFieldFloat32(e, t, n) {
			(this.force_defaults || t != n) && (this.addFloat32(t), this.slot(e));
		}
		addFieldFloat64(e, t, n) {
			(this.force_defaults || t != n) && (this.addFloat64(t), this.slot(e));
		}
		addFieldOffset(e, t, n) {
			(this.force_defaults || t != n) && (this.addOffset(t), this.slot(e));
		}
		addFieldStruct(e, t, n) {
			t != n && (this.nested(t), this.slot(e));
		}
		nested(e) {
			if (e != this.offset()) throw TypeError("FlatBuffers: struct must be serialized inline.");
		}
		notNested() {
			if (this.isNested) throw TypeError("FlatBuffers: object serialization must not be nested.");
		}
		slot(e) {
			this.vtable !== null && (this.vtable[e] = this.offset());
		}
		offset() {
			return this.bb.capacity() - this.space;
		}
		static growByteBuffer(e) {
			let t = e.capacity();
			if (t & 3221225472) throw Error("FlatBuffers: cannot grow buffer beyond 2 gigabytes.");
			let r = t << 1, i = n.ByteBuffer.allocate(r);
			return i.setPosition(r - t), i.bytes().set(e.bytes(), r - t), i;
		}
		addOffset(e) {
			this.prep(r.SIZEOF_INT, 0), this.writeInt32(this.offset() - e + r.SIZEOF_INT);
		}
		startObject(e) {
			this.notNested(), this.vtable ??= [], this.vtable_in_use = e;
			for (let t = 0; t < e; t++) this.vtable[t] = 0;
			this.isNested = !0, this.object_start = this.offset();
		}
		endObject() {
			if (this.vtable == null || !this.isNested) throw Error("FlatBuffers: endObject called without startObject");
			this.addInt32(0);
			let e = this.offset(), t = this.vtable_in_use - 1;
			for (; t >= 0 && this.vtable[t] == 0; t--);
			let n = t + 1;
			for (; t >= 0; t--) this.addInt16(this.vtable[t] == 0 ? 0 : e - this.vtable[t]);
			this.addInt16(e - this.object_start);
			let i = (n + 2) * r.SIZEOF_SHORT;
			this.addInt16(i);
			let a = 0, o = this.space;
			outer_loop: for (t = 0; t < this.vtables.length; t++) {
				let e = this.bb.capacity() - this.vtables[t];
				if (i == this.bb.readInt16(e)) {
					for (let t = r.SIZEOF_SHORT; t < i; t += r.SIZEOF_SHORT) if (this.bb.readInt16(o + t) != this.bb.readInt16(e + t)) continue outer_loop;
					a = this.vtables[t];
					break;
				}
			}
			return a ? (this.space = this.bb.capacity() - e, this.bb.writeInt32(this.space, a - e)) : (this.vtables.push(this.offset()), this.bb.writeInt32(this.bb.capacity() - e, this.offset() - e)), this.isNested = !1, e;
		}
		finish(e, t, n) {
			let i = n ? r.SIZE_PREFIX_LENGTH : 0;
			if (t) {
				let e = t;
				if (this.prep(this.minalign, r.SIZEOF_INT + r.FILE_IDENTIFIER_LENGTH + i), e.length != r.FILE_IDENTIFIER_LENGTH) throw TypeError("FlatBuffers: file identifier must be length " + r.FILE_IDENTIFIER_LENGTH);
				for (let t = r.FILE_IDENTIFIER_LENGTH - 1; t >= 0; t--) this.writeInt8(e.charCodeAt(t));
			}
			this.prep(this.minalign, r.SIZEOF_INT + i), this.addOffset(e), i && this.addInt32(this.bb.capacity() - this.space), this.bb.setPosition(this.space);
		}
		finishSizePrefixed(e, t) {
			this.finish(e, t, !0);
		}
		requiredField(e, t) {
			let n = this.bb.capacity() - e, r = n - this.bb.readInt32(n);
			if (!(t < this.bb.readInt16(r) && this.bb.readInt16(r + t) != 0)) throw TypeError("FlatBuffers: field " + t + " must be set");
		}
		startVector(e, t, n) {
			this.notNested(), this.vector_num_elems = t, this.prep(r.SIZEOF_INT, e * t), this.prep(n, e * t);
		}
		endVector() {
			return this.writeInt32(this.vector_num_elems), this.offset();
		}
		createSharedString(e) {
			if (!e) return 0;
			if (this.string_maps ||= /* @__PURE__ */ new Map(), this.string_maps.has(e)) return this.string_maps.get(e);
			let t = this.createString(e);
			return this.string_maps.set(e, t), t;
		}
		createString(e) {
			if (e == null) return 0;
			let t;
			return t = e instanceof Uint8Array ? e : this.text_encoder.encode(e), this.addInt8(0), this.startVector(1, t.length, 1), this.bb.setPosition(this.space -= t.length), this.bb.bytes().set(t, this.space), this.endVector();
		}
		createByteVector(e) {
			return e == null ? 0 : (this.startVector(1, e.length, 1), this.bb.setPosition(this.space -= e.length), this.bb.bytes().set(e, this.space), this.endVector());
		}
		createObjectOffset(e) {
			return e === null ? 0 : typeof e == "string" ? this.createString(e) : e.pack(this);
		}
		createObjectOffsetList(e) {
			let t = [];
			for (let n = 0; n < e.length; ++n) {
				let r = e[n];
				if (r !== null) t.push(this.createObjectOffset(r));
				else throw TypeError("FlatBuffers: Argument for createObjectOffsetList cannot contain null.");
			}
			return t;
		}
		createStructOffsetList(e, t) {
			return t(this, e.length), this.createObjectOffsetList(e.slice().reverse()), this.endVector();
		}
	};
})), o = (/* @__PURE__ */ e(((e) => {
	Object.defineProperty(e, "__esModule", { value: !0 }), e.Encoding = e.ByteBuffer = e.Builder = e.isLittleEndian = e.int32 = e.float64 = e.float32 = e.SIZE_PREFIX_LENGTH = e.SIZEOF_SHORT = e.SIZEOF_INT = e.FILE_IDENTIFIER_LENGTH = void 0;
	var o = t();
	Object.defineProperty(e, "FILE_IDENTIFIER_LENGTH", {
		enumerable: !0,
		get: function() {
			return o.FILE_IDENTIFIER_LENGTH;
		}
	}), Object.defineProperty(e, "SIZEOF_INT", {
		enumerable: !0,
		get: function() {
			return o.SIZEOF_INT;
		}
	}), Object.defineProperty(e, "SIZEOF_SHORT", {
		enumerable: !0,
		get: function() {
			return o.SIZEOF_SHORT;
		}
	}), Object.defineProperty(e, "SIZE_PREFIX_LENGTH", {
		enumerable: !0,
		get: function() {
			return o.SIZE_PREFIX_LENGTH;
		}
	});
	var s = n();
	Object.defineProperty(e, "float32", {
		enumerable: !0,
		get: function() {
			return s.float32;
		}
	}), Object.defineProperty(e, "float64", {
		enumerable: !0,
		get: function() {
			return s.float64;
		}
	}), Object.defineProperty(e, "int32", {
		enumerable: !0,
		get: function() {
			return s.int32;
		}
	}), Object.defineProperty(e, "isLittleEndian", {
		enumerable: !0,
		get: function() {
			return s.isLittleEndian;
		}
	});
	var c = a();
	Object.defineProperty(e, "Builder", {
		enumerable: !0,
		get: function() {
			return c.Builder;
		}
	});
	var l = i();
	Object.defineProperty(e, "ByteBuffer", {
		enumerable: !0,
		get: function() {
			return l.ByteBuffer;
		}
	});
	var u = r();
	Object.defineProperty(e, "Encoding", {
		enumerable: !0,
		get: function() {
			return u.Encoding;
		}
	});
})))();
function s({ Builder: e, ByteBuffer: t }) {
	function n(e, t) {
		let n = e.createString(t.name);
		e.startVector(4, t.values.length, 4);
		for (let n = t.values.length - 1; n >= 0; n--) e.addInt32(t.values[n]);
		let r = e.endVector(), i = e.createByteVector(t.payload), a = 0;
		if (t.child) {
			let n = e.createString(t.child.label);
			e.startObject(2), e.addFieldInt32(0, t.child.count, 0), e.addFieldOffset(1, n, 0), a = e.endObject();
		}
		return e.startObject(7), e.addFieldInt64(0, t.id, 0n), e.addFieldOffset(1, n, 0), e.addFieldOffset(2, r, 0), e.addFieldOffset(3, i, 0), e.addFieldInt8(4, Number(t.enabled), 1), e.addFieldFloat64(5, t.weight, 0), a && e.addFieldOffset(6, a, 0), e.finish(e.endObject(), "WLZF"), e.asUint8Array().slice();
	}
	let r = (e, t, n) => {
		let r = e.__offset(t, 4 + n * 2);
		return r ? t + r : 0;
	}, i = (e, t, n) => {
		let i = r(e, t, n);
		return i ? e.__string(i) : "";
	};
	return {
		encode: (t) => n(new e(64), t),
		createEncoder() {
			let t = new e(64);
			return (e) => (t.clear(), n(t, e));
		},
		decode(e) {
			let n = new t(e);
			if (n.getBufferIdentifier() !== "WLZF") throw Error("wrong FlatBuffers identifier");
			let a = n.readUint32(0), o = r(n, a, 0), s = r(n, a, 2), c = r(n, a, 3), l = r(n, a, 4), u = r(n, a, 5), d = r(n, a, 6), f = d ? n.__indirect(d) : 0;
			return {
				id: o ? n.readInt64(o) : 0n,
				name: i(n, a, 1),
				values: s ? new Int32Array(e.buffer, e.byteOffset + n.__vector(s), n.__vector_len(s)).slice() : /* @__PURE__ */ new Int32Array(),
				payload: c ? e.slice(n.__vector(c), n.__vector(c) + n.__vector_len(c)) : /* @__PURE__ */ new Uint8Array(),
				enabled: !l || !!n.readUint8(l),
				weight: u ? n.readFloat64(u) : 0,
				child: f ? {
					count: r(n, f, 0) ? n.readInt32(r(n, f, 0)) : 0,
					label: i(n, f, 1)
				} : void 0
			};
		}
	};
}
// benchmarks/flatbuffers/20261111-tuning-v1/size-before-v2/reference.entry.mjs
var { encode: c, decode: l, createEncoder: u } = s({
	Builder: o.Builder,
	ByteBuffer: o.ByteBuffer
});
export { u as createEncoder, l as decode, c as encode };

//# sourceMappingURL=walz-runtime.js.map