import { n as e } from "./rolldown-runtime-CYdcPHCu.js";
// benchmarks/flatbuffers/20261111-tuning-v1/size-after-v2/core-O3/resources.mjs
var t = class {
	constructor(e, t, n = () => 0) {
		this.active = e, this.disposal = t, this.owner = n, this.entries = /* @__PURE__ */ new Map(), this.next = 1;
	}
	add(e, t, n) {
		let r = this.active();
		if (!r) throw TypeError("resource factory requires an active task");
		if (this.next > 2147483647) throw n(t), RangeError("resource IDs exhausted");
		if (t && typeof t.then == "function") throw this.disposal(async () => n(await t)), TypeError("resource factory must be synchronous");
		let i = this.next++;
		return this.entries.set(i, {
			kind: e,
			value: t,
			close: n,
			scope: r,
			owner: this.owner()
		}), r.resources ??= /* @__PURE__ */ new Set(), r.resources.add(i), i;
	}
	entry(e, t) {
		let n = this.entries.get(t);
		if (!n || n.kind !== e || n.scope !== this.active()) throw TypeError("invalid, closed, or foreign resource");
		return n;
	}
	use(e, t, n, r) {
		return n(this.entry(e, t).value, ...r);
	}
	close(e, t) {
		let n = this.entry(e, t);
		this.entries.delete(t), n.scope.resources.delete(t);
		let r;
		try {
			r = n.close(n.value);
		} catch (e) {
			throw this.failure ??= e, e;
		}
		if (r && typeof r.then == "function") throw this.disposal(() => r), this.failure ??= /* @__PURE__ */ TypeError("resource destructor must be synchronous"), this.failure;
	}
	adopt(e, t) {
		let n = this.entries.get(e);
		if (!n || n.scope !== this.active()) throw TypeError("invalid resource move");
		n.owner = t;
	}
	closeScope(e, t) {
		let n = [];
		for (let r of [...e.resources ?? []].reverse()) {
			let e = this.entries.get(r);
			if (t === void 0 || e.owner === t) try {
				this.close(e.kind, r);
			} catch (e) {
				n.push(e);
			}
		}
		if (n.length) throw AggregateError(n, "resource cleanup failed");
	}
};
// benchmarks/flatbuffers/20261111-tuning-v1/size-after-v2/core-O3/tasks.mjs
function n(e, t) {
	if (!(e instanceof Error) || (t ??= /* @__PURE__ */ new Set(), t.has(e))) return;
	t.add(e);
	try {
		e.stack;
	} catch {}
	n(Object.getOwnPropertyDescriptor(e, "cause")?.value, t);
	let r = Object.getOwnPropertyDescriptor(e, "errors")?.value;
	if (Array.isArray(r)) for (let e of r) n(e, t);
}
var r = class extends Error {
	constructor(e, t = "Task cancelled", r = "TaskCancelledError") {
		super(t, { cause: e }), this.name = r, n(this);
	}
}, i = class extends r {
	constructor(e) {
		super(void 0, `Task timed out after ${e} ms`, "TaskTimeoutError"), this.timeoutMs = e;
	}
}, a = (e) => e instanceof i ? e : new r(e), o = /* @__PURE__ */ new Set([
	"returned",
	"cancelled",
	"failed"
]), s = Object.getPrototypeOf(async function() {});
function c() {
	return Promise.withResolvers();
}
function l(e) {
	return Object.freeze({
		get state() {
			return e.state;
		},
		result: e.result.promise,
		cancel(t) {
			return e.cancel ? e.cancel(t) : e.done.promise;
		}
	});
}
var u = class {
	constructor({ onTaskEvent: e = () => {}, asyncExports: n = [] } = {}) {
		this.notify = e, this.asyncExports = new Set(n), this.queue = [], this.active = null, this.failure = null, this.nextId = 1, this.resources = new t(() => this.active, (e) => this.disposal(e), () => this.scope), this.children = null, this.scope = 0;
	}
	capture(e) {
		return e.exports["scope-owner"] && (this.scopeOwner = e.exports["scope-owner"]), e.exports["free-scope"] && (this.freeScope = e.exports["free-scope"]), e.exports.__child_discard && (this.discardChild = e.exports.__child_discard), e.exports.__scope_cleanup && (this.cleanupScope = e.exports.__scope_cleanup), e.exports.__child_run && (this.childRun = WebAssembly.promising(e.exports.__child_run)), e.exports.__children_clear && (this.clearChildren = e.exports.__children_clear), e.exports["free-all"] && (this.freeBoundary = e.exports["free-all"]), e.exports.__task_cleanup && (this.guestCleanup = e.exports.__task_cleanup), e.exports["cancel-requested"] && (this.flag = e.exports["cancel-requested"]), e.exports.__task_cancel && (this.tag = e.exports.__task_cancel), e;
	}
	enter(e) {
		this.scope = e, this.scopeOwner && (this.scopeOwner.value = e);
	}
	cancelled(e = this.scope) {
		return this.active?.state === "cancelling" || !!this.active?.children?.scopes.get(e)?.cancelled;
	}
	cleanupGuest() {
		this.cleaning = !0;
		try {
			this.guestCleanup?.();
		} finally {
			this.cleaning = !1;
		}
	}
	bind(e) {
		e.walzChildRun && (this.childRun = e.walzChildRun);
		let { walzChildRun: t, ...n } = e;
		if (!this.flag || !this.tag) throw Error("component lacks the cancellation ABI");
		this.raw = e.api;
		for (let [e, t] of Object.entries(this.raw)) typeof t == "function" && Object.getPrototypeOf(t) === s && this.asyncExports.add(e);
		let r = Object.fromEntries(Object.entries(this.raw).map(([e, t]) => [e, typeof t == "function" ? (...t) => this.invoke(e, t) : t]));
		return {
			...n,
			api: r,
			"walz:generated/api@0.1.0": r,
			start: this.start.bind(this)
		};
	}
	create(e, t, n = {}) {
		if (!Object.hasOwn(this.raw, e) || typeof this.raw[e] != "function") throw TypeError(`unknown export ${e}`);
		if (!Array.isArray(t)) throw TypeError("task arguments must be an array");
		if (n.timeoutMs !== void 0 && (!Number.isInteger(n.timeoutMs) || n.timeoutMs < 0 || n.timeoutMs > 2147483647)) throw RangeError("invalid timeoutMs");
		if (n.signal && typeof n.signal.addEventListener != "function") throw TypeError("expected AbortSignal");
		let r = c(), s = c();
		r.promise.catch(() => {});
		let u = {
			id: this.nextId++,
			name: e,
			args: [...t],
			state: "queued",
			started: !1,
			settling: !1,
			reason: void 0,
			result: r,
			done: s,
			disposals: [],
			reads: /* @__PURE__ */ new Set(),
			blocking: /* @__PURE__ */ new Set()
		}, d = (e) => {
			if (o.has(u.state) || u.settling || u.state === "cancelling") return s.promise;
			if (u.reason = e, !u.started) return u.state = "cancelled", u.cancel = void 0, u.detach?.(), u.detach = void 0, u.args = [], r.reject(a(e)), s.resolve({
				status: "cancelled",
				started: !1
			}), s.promise;
			u.state = "cancelling", this.flag.value = 1;
			for (let e of u.reads) e();
			for (let e of u.blocking) e();
			return s.promise;
		};
		u.cancel = d, u.public = l(u);
		let f, p = n.signal, m = () => d(p.reason);
		return u.detach = () => {
			clearTimeout(f), p?.removeEventListener("abort", m);
		}, p?.addEventListener("abort", m, { once: !0 }), p?.aborted && m(), !o.has(u.state) && n.timeoutMs !== void 0 && (n.timeoutMs === 0 ? d(new i(0)) : f = setTimeout(() => d(new i(n.timeoutMs)), n.timeoutMs)), u;
	}
	start(e, t = [], n = {}) {
		let r = this.create(e, t, n);
		return this.queue.push(r), queueMicrotask(() => this.pump()), r.public;
	}
	pump() {
		if (!this.active) for (; this.queue.length;) {
			let e = this.queue.shift();
			if (e.state === "queued") {
				this.execute(e);
				return;
			}
		}
	}
	invoke(e, t) {
		if (this.failure) {
			if (this.asyncExports.has(e)) return Promise.reject(this.failure);
			throw this.failure;
		}
		if (this.active || this.queue.length) {
			if (!this.asyncExports.has(e)) throw Error("component is busy; use start() to queue a synchronous export");
			return this.start(e, t).result;
		}
		let n = this.create(e, t);
		return this.execute(n, !0);
	}
	execute(e, t = !1) {
		this.active = e, e.started = !0, e.state = "running", this.flag.value = 0, this.enter(0);
		let n;
		try {
			if (this.failure) throw this.failure;
			n = this.raw[e.name](...e.args);
		} catch (n) {
			if (e.args = [], this.finish(e, void 0, n), t && !this.asyncExports.has(e.name)) throw n;
			return e.result.promise;
		}
		if (e.args = [], this.asyncExports.has(e.name)) return Promise.resolve(n).then((t) => this.finish(e, t), (t) => this.finish(e, void 0, t)), e.result.promise;
		try {
			this.resources.closeScope(e);
		} catch (n) {
			if (this.failure = n, this.finish(e, void 0, n), t) throw n;
			return e.result.promise;
		}
		return e.disposals.length ? (this.finish(e, n), t ? n : e.result.promise) : (this.publish(e, n), n);
	}
	async finish(e, t, n) {
		e.settling = !0;
		try {
			let t = [];
			if (e.children && (await this.children.finish(e, n), n = e.childFailure ?? n), n) try {
				this.cleanupGuest();
			} catch (e) {
				t.push(e);
			}
			for (; e.disposals.length;) {
				let n = await Promise.allSettled(e.disposals.splice(0));
				t.push(...n.filter((e) => e.status === "rejected").map((e) => e.reason));
			}
			try {
				this.resources.closeScope(e);
			} catch (e) {
				t.push(e);
			}
			for (; e.disposals.length;) {
				let n = await Promise.allSettled(e.disposals.splice(0));
				t.push(...n.filter((e) => e.status === "rejected").map((e) => e.reason));
			}
			if (this.resources.failure && t.push(this.resources.failure), t.length) throw AggregateError(t, "task cleanup failed");
		} catch (e) {
			this.failure = e, n = e;
		}
		if (n) try {
			this.freeBoundary?.();
		} catch (e) {
			n = AggregateError([n, e], "boundary cleanup failed"), this.failure = n;
		}
		let r = e.state === "cancelling" && n instanceof WebAssembly.Exception && n.is(this.tag);
		this.publish(e, t, n, r);
	}
	publish(e, t, r, i = !1) {
		n(r), e.state = i ? "cancelled" : r ? "failed" : "returned", e.cancel = void 0, e.detach?.(), e.detach = void 0, i ? e.result.reject(a(e.reason)) : r ? e.result.reject(r) : typeof t?.resolveAsValue == "function" ? t.resolveAsValue(e.result.resolve) : e.result.resolve(t), e.done.resolve({
			status: e.state,
			started: e.started
		}), this.active = null, this.flag.value = 0, queueMicrotask(() => this.pump());
	}
	disposal(e) {
		let t = Promise.resolve().then(e);
		return this.active ? (this.active.disposals.push(t), (this.active.children?.scopes.get(this.scope))?.disposals.push(t)) : t.catch((e) => console.error("host disposal failed", e)), t.catch(() => {}), t;
	}
	read(e, t) {
		let n = this.scope, r = t();
		if (!r || typeof r.then != "function") return r;
		let i = this.active;
		return i ? (e.scope = n, i.reads.add(e), this.notify({
			type: "read-pending",
			id: i.id
		}), this.cancelled(n) && e(), Promise.resolve(r).finally(() => {
			this.enter(n), i.reads.delete(e);
		})) : r;
	}
	host(e, t = ["sleep", "yield"]) {
		return Object.fromEntries(Object.entries(e).map(([n, r]) => [n, typeof r == "function" ? (...i) => {
			let a = this.scope, o = r.apply(e, i), s = this.active;
			if (!s || !t.includes(n)) return o;
			let c = !1, l, u = () => {
				if (c) return;
				c = !0;
				let e = o?.[Symbol.asyncDispose] ?? o?.[Symbol.dispose];
				e && (l = this.disposal(() => e.call(o)));
			};
			u.scope = a, s.blocking.add(u), this.notify({
				type: "host-pending",
				id: s.id,
				name: n
			}), this.cancelled(a) && u();
			let d = async () => {
				throw await l, new WebAssembly.Exception(this.tag, []);
			};
			return Promise.resolve(o).then((e) => (this.enter(a), this.cancelled(a) ? d() : e), (e) => {
				if (this.enter(a), this.cancelled(a)) return d();
				throw e;
			}).finally(() => s.blocking.delete(u));
		} : r]));
	}
};
// benchmarks/flatbuffers/20261111-tuning-v1/size-after-v2/core-O3/host.mjs
function d({ onEvent: e = () => {} } = {}) {
	function t(t, n) {
		if (!Number.isInteger(t) || t < 0 || t > 2147483647) throw new WebAssembly.RuntimeError("invalid timer duration");
		let r, i = !0, a, o = new Promise((o) => {
			r = o, a = setTimeout(() => {
				i = !1, e({
					type: "timer",
					milliseconds: t
				}), o(n);
			}, t);
		}), s = () => {
			i && (i = !1, clearTimeout(a), e({
				type: "dispose",
				milliseconds: t
			}), r(n));
		};
		return o[Symbol.asyncDispose] = s, o[Symbol.dispose] = s, e({
			type: "timer-start",
			milliseconds: t
		}), {
			promise: o,
			cancel: s
		};
	}
	return {
		after(e) {
			return t(e, void 0).promise;
		},
		sleep(e) {
			return t(e, void 0).promise;
		},
		yield() {
			return t(0, void 0).promise;
		},
		deferI64(e, n) {
			return t(e, n).promise;
		},
		fallibleI64(e, n) {
			return t(e, n === 0 ? {
				tag: "ok",
				val: 42n
			} : {
				tag: "err",
				val: n
			}).promise;
		},
		ticks(n, r) {
			if (!Number.isInteger(n) || n < 0 || n > 65536 || !Number.isInteger(r) || r < 0 || r > 2147483647) throw new WebAssembly.RuntimeError("invalid bounded stream");
			let i = 0, a = !1, o;
			return {
				[Symbol.asyncIterator]() {
					return this;
				},
				async next() {
					if (a || i === n) return { done: !0 };
					if (o) throw TypeError("concurrent stream read");
					let e = t(r, void 0);
					o = e;
					try {
						return await e.promise, a ? { done: !0 } : {
							done: !1,
							value: BigInt(i++)
						};
					} finally {
						o === e && (o = void 0);
					}
				},
				async return() {
					return a || (a = !0, o?.cancel(), e({ type: "stream-close" })), { done: !0 };
				}
			};
		}
	};
}
function f(e, t, n = []) {
	let r = "walz:host/event-loop@0.1.0", i = Object.assign(Object.create(null), e.imports);
	i[r] = t.host(i[r] ?? e.eventLoop ?? d());
	let a = { ...i };
	for (let e of n.filter((e) => e.resource)) {
		let { kind: r, operation: o } = e.resource, s = a[e.module], c = s?.[e.jsName];
		if (typeof c != "function") throw TypeError(`missing resource provider ${e.module}#${e.name}`);
		let l = (...e) => c.apply(s, e);
		if (i[e.module] = { ...i[e.module] }, o === "new") {
			let o = n.find((e) => e.resource?.kind === r && e.resource.operation === "close"), s = a[o?.module], c = s?.[o?.jsName];
			if (typeof c != "function") throw TypeError(`missing destructor for ${r}`);
			i[e.module][e.jsName] = (...e) => t.resources.add(r, l(...e), (e) => c.call(s, e));
		} else if (o === "use" || o === "reader") i[e.module][e.jsName] = (e, ...n) => t.resources.use(r, e, l, n);
		else if (o === "close") i[e.module][e.jsName] = (e) => t.resources.close(r, e);
		else throw TypeError(`unsupported resource operation ${o}`);
	}
	for (let e of new Set(n.map((e) => e.module))) e !== r && i[e] && (i[e] = t.host(i[e], n.filter((t) => t.module === e && t.effects.suspends).map((e) => e.jsName)));
	for (let e of n) if (typeof i[e.module]?.[e.jsName] != "function") throw TypeError(`missing host capability ${e.module}#${e.name}`);
	for (let [e, t] of Object.entries(i)) i[e.replace(/@[^@]+$/, "")] = t;
	return i;
}
// benchmarks/flatbuffers/20261111-tuning-v1/size-after-v2/core-O3/core.mjs
var p = () => {}, m = class {
	constructor(e) {
		this.source = e;
	}
	then(e, t) {
		return Promise.resolve(this.source).then(e, t);
	}
	resolveAsValue(e) {
		Object.defineProperty(this, "then", {
			value: void 0,
			configurable: !0
		});
		try {
			e(this);
		} finally {
			delete this.then;
		}
	}
	[Symbol.asyncDispose]() {
		let e = this.source;
		return (e?.[Symbol.asyncDispose] ?? e?.[Symbol.dispose])?.call(e);
	}
}, h = class {
	constructor(e, t, n) {
		this.allocator = e.exports, this.memory = this.allocator.memory, this.tasks = t, this.hosts = n, this.imports = { ...e.exports }, this.definitions = /* @__PURE__ */ new Map(), this.handles = /* @__PURE__ */ new Map(), this.nextHandle = 1, this.promising = Object.create(null);
	}
	view() {
		return new DataView(this.memory.buffer);
	}
	alloc(e) {
		return this.allocator.alloc(e);
	}
	failHost(e, t) {
		throw e instanceof WebAssembly.Exception && e.is(this.tasks.tag) ? e : (this.hostFailure ??= e instanceof Error && !t ? e : new WebAssembly.RuntimeError(t ?? "host operation failed", { cause: e }), this.hostFailure);
	}
	add(e, t) {
		if (this.definitions.get(e).kind === "stream") t = this.wrapStream(t);
		else {
			if (typeof t?.then != "function") throw TypeError("expected PromiseLike");
			Promise.resolve(t).catch(p);
		}
		if (this.nextHandle > 4294967295) throw RangeError("endpoint IDs exhausted");
		let n = this.nextHandle++;
		return this.handles.set(n, {
			type: e,
			source: t
		}), n;
	}
	entry(e, t) {
		let n = this.handles.get(t >>> 0);
		if (!n || n.type !== e) throw TypeError("invalid readable handle");
		return n;
	}
	take(e, t) {
		t >>>= 0;
		let { source: n } = this.entry(e, t);
		return this.handles.delete(t), this.definitions.get(e).kind === "future" ? new m(n) : n;
	}
	drop(e, t) {
		t >>>= 0;
		let { source: n } = this.entry(e, t);
		this.handles.delete(t);
		let r = this.definitions.get(e).kind === "stream" ? n.return : n?.[Symbol.asyncDispose] ?? n?.[Symbol.dispose];
		r && this.tasks.disposal(() => r.call(n));
	}
	hostImport(e, t, n, r, i = "walz:host/event-loop@0.1.0", a = (e) => e) {
		let o = this.hosts[i];
		if (typeof o?.[t] != "function") throw TypeError(`missing host capability ${i}#${t}`);
		let s = (...e) => {
			let i = this.tasks.scope;
			if (this.hostFailure) throw this.hostFailure;
			try {
				let s = o[t](...a(e));
				return n ? Promise.resolve(s).then((t) => (this.tasks.enter(i), r(t, e))).catch((e) => this.failHost(e)) : r(s, e);
			} catch (e) {
				return this.failHost(e);
			}
		};
		this.imports[e] = n ? new WebAssembly.Suspending(s) : s;
	}
	call(e, t, n, r, i, a, o, s) {
		if (t !== s) throw TypeError(`expected ${t} arguments`);
		let c = this.nextHandle, l = 0;
		try {
			t && (l = this.alloc(n)), r(l);
		} catch (e) {
			for (let [e, t] of this.handles) e >= c && this.drop(t.type, e);
			throw this.allocator["free-all"](), e;
		}
		let u = (e) => {
			if (this.hostFailure) throw this.hostFailure;
			if (a) try {
				return i(e);
			} finally {
				this.exports[a](e);
			}
		}, d = (e) => {
			if (!(e instanceof WebAssembly.Exception && e.is(this.tasks.tag))) {
				if (this.tasks.failure = Error("cannot enter component instance after failure", { cause: e }), this.tasks.active?.children) throw e;
				try {
					this.tasks.cleanupGuest();
				} catch (t) {
					e = AggregateError([e, t], "guest cleanup failed");
				}
				for (let [e, t] of this.handles) this.drop(t.type, e);
				this.allocator["free-all"]();
			}
			throw e;
		}, f = t ? [l] : [];
		try {
			return o ? (this.promising[e] ??= WebAssembly.promising(this.exports[e]))(...f).then(u).catch(d) : u(this.exports[e](...f));
		} catch (e) {
			return d(e);
		}
	}
};
async function g(t, n, r, i = {}, a = [], o = [], s = null, c = null, l = null, d = null) {
	if (![
		"walz-core-js-v1",
		"walz-core-js-v2",
		"walz-core-js-v3",
		"walz-core-js-v4"
	].includes(n)) throw Error("unsupported Core JS ABI");
	if (n === "walz-core-js-v3" || n === "walz-core-js-v4") {
		if (!d) throw Error("shared-memory contract validator is required");
		d(n, s, o, i);
	} else if (s !== null) throw Error("shared-memory contract requires Core JS ABI v3 or v4");
	let p = i.getCoreModule ?? (async (n) => {
		let r = new URL(n, t);
		return r.protocol === "file:" && typeof process < "u" && process.versions?.node ? WebAssembly.compile(await (await import("./__vite-browser-external-q4xEsQeL.js").then((t) => /* @__PURE__ */ e(t.default, 1))).readFile(r)) : WebAssembly.compileStreaming(fetch(r));
	}), m = new u(i), g = f(i, m, a), _ = async (e, t) => m.capture(await (i.instantiateCore ?? WebAssembly.instantiate)(await p(e), t)), v = new h(await _("memory.wasm", {}), m, g);
	l?.(v);
	let { api: y, asyncExports: b, childTasks: x } = r(v);
	if (x) {
		if (!c) throw Error("child task scheduler is required");
		c.core(m, v);
	}
	m.asyncExports = new Set(b);
	let S = Object.create(null);
	S.__component = v.imports;
	for (let { module: e, name: t } of o) {
		if (e === "__component") throw Error("__component is reserved for private ABI imports");
		let n = Object.hasOwn(S, e) ? S[e] : S[e] = Object.create(null);
		n[t] = i.imports?.[e]?.[t];
	}
	return v.exports = (await _("core.wasm", S)).exports, m.bind({
		api: y,
		__memory: () => ({ handles: v.handles.size })
	});
}
// benchmarks/flatbuffers/20261111-tuning-v1/size-after-v2/core-O3/bindings.mjs
function _(e) {
	let t = new Uint8Array(new Uint16Array([1]).buffer)[0] === 1, n = Object.getPrototypeOf(Uint8Array.prototype), r = Object.getOwnPropertyDescriptor(n, Symbol.toStringTag).get, i = Object.getOwnPropertyDescriptor(n, "length").get;
	function a(t) {
		return e.view().getInt32(t, !0);
	}
	function o(t, n) {
		if (!Number.isInteger(n) || n < -2147483648 || n > 2147483647) throw TypeError("expected s32");
		e.view().setInt32(t, n, !0);
	}
	function s(t) {
		return e.view().getBigInt64(t, !0);
	}
	function c(t, n) {
		if (typeof n != "bigint" || n < -9223372036854775808n || n > 9223372036854775807n) throw TypeError("expected s64");
		e.view().setBigInt64(t, n, !0);
	}
	function l(t) {
		return e.view().getFloat64(t, !0);
	}
	function u(t, n) {
		if (typeof n != "number") throw TypeError("expected f64");
		e.view().setFloat64(t, n, !0);
	}
	function d(t) {
		let n = e.view().getUint8(t);
		if (n > 1) throw TypeError("invalid bool");
		return !!n;
	}
	function f(t, n) {
		if (typeof n != "boolean") throw TypeError("expected bool");
		e.view().setUint8(t, +!!n);
	}
	function p(e) {
		return {
			count: a(e + 0),
			label: S(e + 4)
		};
	}
	function m(e, t) {
		if (typeof t != "object" || !t || Array.isArray(t)) throw TypeError("expected record");
		o(e + 0, t.count), C(e + 4, t.label);
	}
	function h(e) {
		return {
			id: s(e + 0),
			name: S(e + 8),
			values: b(e + 16),
			payload: _(e + 24),
			enabled: d(e + 32),
			weight: l(e + 40),
			child: w(e + 48)
		};
	}
	function g(e, t) {
		if (typeof t != "object" || !t || Array.isArray(t)) throw TypeError("expected record");
		c(e + 0, t.id), C(e + 8, t.name), x(e + 16, t.values), v(e + 24, t.payload), f(e + 32, t.enabled), u(e + 40, t.weight), T(e + 48, t.child);
	}
	function _(t) {
		let n = e.view().getUint32(t, !0), r = e.view().getUint32(t + 4, !0);
		return new Uint8Array(e.memory.buffer, n, r).slice();
	}
	function v(t, n) {
		if (!(n instanceof Uint8Array)) throw TypeError("expected Uint8Array");
		let r = n, i = r.length ? e.alloc(r.length) : 0;
		new Uint8Array(e.memory.buffer, i, r.length).set(r), e.view().setUint32(t, i, !0), e.view().setUint32(t + 4, r.length, !0);
	}
	function y(t) {
		let n = e.view().getUint8(t);
		if (n === 0) return {
			tag: "ok",
			val: h(t + 8)
		};
		if (n === 1) return {
			tag: "err",
			val: a(t + 8)
		};
		throw TypeError("invalid result");
	}
	function b(n) {
		let r = e.view().getUint32(n, !0), i = e.view().getUint32(n + 4, !0);
		if (i > 4194304 || r % 4 || r + i * 4 > e.memory.buffer.byteLength) throw TypeError("invalid list bounds");
		if (t) return new Int32Array(new Int32Array(e.memory.buffer, r, i));
		let o = new Int32Array(i);
		for (let e = 0; e < i; e++) o[e] = a(r + e * 4);
		return o;
	}
	function x(n, a) {
		if (!(Array.isArray(a) || a instanceof Int32Array) || a.length > 4194304) throw TypeError("expected bounded list");
		let s = a.length, c = s ? e.alloc(s * 4) : 0;
		if (e.view().setUint32(n, c, !0), e.view().setUint32(n + 4, s, !0), t && r.call(a) === "Int32Array" && i.call(a) === s) s && new Int32Array(e.memory.buffer, c, s).set(a);
		else for (let e = 0; e < s; e++) o(c + e * 4, a[e]);
	}
	function S(t) {
		let n = e.view().getUint32(t, !0), r = e.view().getUint32(t + 4, !0), i = new Uint8Array(e.memory.buffer, n, r).slice();
		return new TextDecoder("utf-8", {
			fatal: !0,
			ignoreBOM: !0
		}).decode(i);
	}
	function C(t, n) {
		if (typeof n != "string") throw TypeError("expected string");
		let r = new TextEncoder().encode(n), i = r.length ? e.alloc(r.length) : 0;
		new Uint8Array(e.memory.buffer, i, r.length).set(r), e.view().setUint32(t, i, !0), e.view().setUint32(t + 4, r.length, !0);
	}
	function w(t) {
		let n = e.view().getUint8(t);
		if (n !== 0) {
			if (n !== 1) throw TypeError("invalid option");
			return p(t + 4);
		}
	}
	function T(t, n) {
		n === void 0 ? e.view().setUint8(t, 0) : (e.view().setUint8(t, 1), m(t + 4, n));
	}
	let E = Object.create(null);
	return E.encode = (...t) => e.call("__export_0", 1, 64, (e) => {
		g(e + 0, t[0]);
	}, (e) => _(e), "__post_0", !1, t.length), E.decode = (...t) => e.call("__export_1", 1, 8, (e) => {
		v(e + 0, t[0]);
	}, (e) => y(e), "__post_1", !1, t.length), E.main = (...t) => e.call("__export_2", 0, 0, (e) => {}, (e) => a(e), "__post_2", !1, t.length), {
		api: E,
		asyncExports: []
	};
}
// benchmarks/flatbuffers/20261111-tuning-v1/size-after-v2/core-O3/index.js
var v = (e = {}) => g(import.meta.url, "walz-core-js-v2", _, e, [], [], null, null, null, null);
export { v as instantiate };

//# sourceMappingURL=walz-runtime.js.map