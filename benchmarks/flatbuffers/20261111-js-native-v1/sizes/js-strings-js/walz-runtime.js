import { n as e } from "./rolldown-runtime-CYdcPHCu.js";
// benchmarks/flatbuffers/20261111-js-native-v1/sizes/js-strings/runtime.mjs
var t = 16777216, n = Object.getPrototypeOf(Uint8Array.prototype), r = Object.getOwnPropertyDescriptor(n, Symbol.toStringTag).get, i = Object.getOwnPropertyDescriptor(n, "length").get, a = new Uint8Array(Uint16Array.of(1).buffer)[0] === 1, o = String.prototype.toWellFormed;
function s(e, t = /* @__PURE__ */ new Set()) {
	if (!(e instanceof Error) || t.has(e)) return;
	t.add(e);
	try {
		e.stack;
	} catch {}
	s(Object.getOwnPropertyDescriptor(e, "cause")?.value, t);
	let n = Object.getOwnPropertyDescriptor(e, "errors")?.value;
	if (Array.isArray(n)) for (let e of n) s(e, t);
}
async function c(n, c, l, u = {}) {
	if (l.abi !== "walz-js-native-experiment-v1") throw Error("unsupported native ABI");
	let d = l.jsStrings ? {
		builtins: ["js-string"],
		importedStringConstants: "walz:strings"
	} : {}, f = await (u.getCoreModule ?? (async (t) => {
		let r = new URL(t, n);
		return r.protocol === "file:" ? WebAssembly.compile(await (await import("./__vite-browser-external-q4xEsQeL.js").then((t) => /* @__PURE__ */ e(t.default, 1))).readFile(r), d) : WebAssembly.compileStreaming(fetch(r), d);
	}))("native.wasm", d);
	if (l.jsStrings && WebAssembly.Module.imports(f).some((e) => e.module === "wasm:js-string" || e.module === "walz:strings")) throw Error("JS string builtins/imported constants are required");
	if (l.jsStrings && typeof o != "function") throw Error("String.toWellFormed is required");
	let p = new TextEncoder(), m = new TextDecoder("utf-8", {
		fatal: !0,
		ignoreBOM: !0
	}), h, g = !1, _, v = !1, y, b = (e) => (...t) => {
		try {
			return e(...t);
		} catch (e) {
			throw y ??= e, e;
		}
	}, x = {
		get memory() {
			return h;
		},
		reserve(e) {
			if (!Number.isInteger(e) || e < 0 || e > t) throw RangeError("transfer limit");
			e > h.buffer.byteLength && h.grow(Math.ceil(e / 65536) - h.buffer.byteLength / 65536);
		},
		writeBytes(e) {
			if (r.call(e) !== "Uint8Array") throw TypeError("expected Uint8Array");
			let t = i.call(e);
			return x.reserve(t), new Uint8Array(h.buffer, 0, t).set(e), t;
		},
		writeText(e) {
			if (typeof e != "string") throw TypeError("expected string");
			return x.writeBytes(p.encode(e));
		},
		wellFormed(e) {
			return o.call(e);
		},
		readBytes(e) {
			return x.reserve(e), new Uint8Array(h.buffer, 0, e).slice();
		},
		decode(e) {
			return x.reserve(e), m.decode(new Uint8Array(h.buffer, 0, e));
		},
		arrayLength(e, t) {
			if (!Array.isArray(e)) throw TypeError("expected list");
			let n = e.length;
			if (!Number.isInteger(n) || n < 0 || n > t) throw RangeError("list limit");
			return n;
		},
		writeList(e, n, o, s, c, l) {
			let u = Array.isArray(e), d = u ? x.arrayLength(e, t / s) : i.call(e);
			if (!u && r.call(e) !== o) throw TypeError("expected " + o);
			if (x.reserve(d * s), !u && a) new n(h.buffer, 0, d).set(e);
			else {
				let t = new DataView(h.buffer);
				for (let n = 0; n < d; n++) t["set" + c](n * s, l(e[n]), !0);
			}
			return d;
		},
		readList(e, t, n, r) {
			if (x.reserve(e * n), a) return new t(new t(h.buffer, 0, e));
			let i = new DataView(h.buffer), o = new t(e);
			for (let t = 0; t < e; t++) o[t] = i["get" + r](t * n, !0);
			return o;
		},
		invoke(e) {
			v = !0;
			let t = e();
			if (y) throw y;
			return t;
		},
		run(e) {
			if (_) throw _;
			if (g) throw Error("native instance is busy");
			g = !0, v = !1;
			try {
				return e();
			} catch (e) {
				throw v && (_ = Error("native instance failed", { cause: e }), s(_)), e;
			} finally {
				g = !1;
			}
		}
	}, S = l.jsStrings ? { __native: {
		encode: b((e) => x.writeText(e)),
		decode: b((e) => x.decode(e))
	} } : {}, C = await (u.instantiateCore ?? WebAssembly.instantiate)(f, S);
	return h = C.exports.__transfer, {
		api: c(C.exports, x),
		__memory: () => ({
			hostHandles: 0,
			boundaryBuffers: 0,
			scratchCapacityBytes: h.buffer.byteLength
		})
	};
}
// benchmarks/flatbuffers/20261111-js-native-v1/sizes/js-strings/bindings.mjs
function l(e, t) {
	function n(e) {
		return e;
	}
	function r(e) {
		if (!Number.isInteger(e) || e < -2147483648 || e > 2147483647) throw TypeError("expected s32");
		return e;
	}
	function i(e) {
		return e;
	}
	function a(e) {
		if (typeof e != "bigint" || e < -9223372036854775808n || e > 9223372036854775807n) throw TypeError("expected s64");
		return e;
	}
	function o(e) {
		return e;
	}
	function s(e) {
		if (typeof e != "number") throw TypeError("expected float");
		return e;
	}
	function c(e) {
		if (e !== 0 && e !== 1) throw TypeError("invalid bool");
		return !!e;
	}
	function l(e) {
		if (typeof e != "boolean") throw TypeError("expected bool");
		return +!!e;
	}
	function u(t) {
		return {
			count: n(e.get_r8_0(t)),
			label: y(e.get_r8_1(t))
		};
	}
	function d(t) {
		if (typeof t != "object" || !t || Array.isArray(t)) throw TypeError("expected record");
		return e.make_r8(r(t.count), b(t.label));
	}
	function f(t) {
		return {
			id: i(e.get_r9_0(t)),
			name: y(e.get_r9_1(t)),
			values: _(e.get_r9_2(t)),
			payload: m(e.get_r9_3(t)),
			enabled: c(e.get_r9_4(t)),
			weight: o(e.get_r9_5(t)),
			child: x(e.get_r9_6(t))
		};
	}
	function p(t) {
		if (typeof t != "object" || !t || Array.isArray(t)) throw TypeError("expected record");
		return e.make_r9(a(t.id), b(t.name), v(t.values), h(t.payload), l(t.enabled), s(t.weight), S(t.child));
	}
	function m(n) {
		return t.readBytes(e.get_v10(n));
	}
	function h(n) {
		return e.make_v10(t.writeBytes(n));
	}
	function g(t) {
		return e.tag_v11(t) === 0 ? {
			tag: "ok",
			val: f(e.get_v11_0_0(t))
		} : {
			tag: "err",
			val: n(e.get_v11_1_0(t))
		};
	}
	function _(n) {
		return t.readList(e.get_v12(n), Int32Array, 4, "Int32");
	}
	function v(n) {
		let i = t.writeList(n, Int32Array, "Int32Array", 4, "Int32", r);
		return e.make_v12(i);
	}
	function y(t) {
		return e.get_v13(t);
	}
	function b(n) {
		if (typeof n != "string") throw TypeError("expected string");
		return e.make_v13(t.wellFormed(n));
	}
	function x(t) {
		return e.tag_v14(t) === 0 ? void 0 : u(e.get_v14_1_0(t));
	}
	function S(t) {
		return t === void 0 ? e.make_v14_0() : e.make_v14_1(d(t));
	}
	let C = Object.create(null);
	return C.encode = (...n) => t.run(() => {
		if (n.length !== 1) throw TypeError("argument count");
		let r = p(n[0]);
		return m(t.invoke(() => e.__call_0(r)));
	}), C.decode = (...n) => t.run(() => {
		if (n.length !== 1) throw TypeError("argument count");
		let r = h(n[0]);
		return g(t.invoke(() => e.__call_1(r)));
	}), C.main = (...r) => t.run(() => {
		if (r.length !== 0) throw TypeError("argument count");
		return n(t.invoke(() => e.__call_2()));
	}), C;
}
// benchmarks/flatbuffers/20261111-js-native-v1/sizes/js-strings/index.js
var u = {
	abi: "walz-js-native-experiment-v1",
	jsStrings: !0,
	maximumTransferBytes: 16777216,
	note: "Experimental shared-value JS ABI, not Canonical ABI; no start/cancellation/host imports.",
	synchronous: !0
}, d = (e = {}) => c(import.meta.url, l, u, e);
export { d as instantiate };

//# sourceMappingURL=walz-runtime.js.map