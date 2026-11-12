import { n as e } from "./rolldown-runtime-CYdcPHCu.js";
// benchmarks/flatbuffers/20261111-three-improvements-v1/sizes/component-strip/component.js
var t, n = [];
function r() {
	return typeof setImmediate == "function" ? new Promise((e) => setImmediate(e)) : typeof MessageChannel == "function" ? new Promise((e) => {
		t || (t = new MessageChannel(), t.port1.onmessage = () => {
			let e = n.splice(0);
			for (let t of e) t();
		}), n.push(e), n.length === 1 && t.port2.postMessage(0);
	}) : new Promise((e) => setTimeout(e, 0));
}
function i(e, t, n, i) {
	let a = i?.shim?.globals?.WebAssembly ?? globalThis.WebAssembly;
	if (n === void 0) {
		if (a === void 0) throw Error("no WebAssembly implementation is available: pass instantiateCore or options.shim.globals.WebAssembly");
		n = (e, t) => a.instantiate(e, t);
	}
	let o = /* @__PURE__ */ new DataView(/* @__PURE__ */ new ArrayBuffer()), s = (e) => o.buffer === e.buffer ? o : o = new DataView(e.buffer);
	function c(e) {
		let t = BigInt(e);
		return BigInt.asIntN(64, t);
	}
	function l(e) {
		return e | 0;
	}
	function u(e, t) {
		if (t == null) return !1;
		switch (e) {
			case "bool": return t === 0 || t === 1;
			case "u8": return typeof t == "number" && Number.isInteger(t) && t >= 0 && t <= 255;
			case "s8": return typeof t == "number" && Number.isInteger(t) && t >= -128 && t <= 127;
			case "u16": return typeof t == "number" && Number.isInteger(t) && t >= 0 && t <= 65535;
			case "s16": return typeof t == "number" && Number.isInteger(t) && t >= -32768 && t <= 32767;
			case "u32": return typeof t == "number" && Number.isInteger(t) && t >= 0 && t <= 4294967295;
			case "s32": return typeof t == "number" && Number.isInteger(t) && t >= -2147483648 && t <= 2147483647;
			case "u64": return typeof t == "bigint" && t >= 0 && t <= 18446744073709551615n;
			case "s64": return typeof t == "bigint" && t >= -9223372036854775808n && t <= 9223372036854775807n;
			case "f32":
			case "f64": return typeof t == "number";
			default: return !1;
		}
		return !0;
	}
	function d(e, t) {
		if (t == null || !u(e, t)) throw TypeError(`invalid ${e} value [${t}]`);
		return !0;
	}
	let f = new TextDecoder("utf-8", {
		fatal: !0,
		ignoreBOM: !0
	}), p = new TextEncoder();
	function m(e, t, n) {
		if (typeof e != "string") throw TypeError("expected a string, received [" + typeof e + "]");
		if (e.length === 0) return {
			ptr: 1,
			len: 0
		};
		let r = 0, i = 0;
		for (let t = 0; t < e.length; t++) {
			let n = e.charCodeAt(t);
			i++, n < 128 ? r += 1 : n < 2048 ? r += 2 : n >= 55296 && n <= 56319 && t + 1 < e.length && (e.charCodeAt(t + 1) & 64512) == 56320 ? (r += 4, t++) : r += 3;
		}
		let a = t(0, 0, 1, r), { read: o, written: s } = p.encodeInto(e, new Uint8Array(n.buffer, a, r));
		if (o !== e.length || s !== r) throw Error("failed to encode whole string");
		return {
			ptr: a,
			len: r,
			codepoints: i
		};
	}
	let h = /* @__PURE__ */ new Map();
	function g(e) {
		let t = h.get(e);
		return t === void 0 && (t = {
			next: 1,
			free: [],
			alloc() {
				return this.free.length > 0 ? this.free.pop() : this.next++;
			},
			release(e) {
				this.free.push(e);
			}
		}, h.set(e, t)), t;
	}
	let _ = 0, v = /* @__PURE__ */ new Map(), y = /* @__PURE__ */ new Map();
	function b() {
		if (Promise.withResolvers) return Promise.withResolvers();
		{
			let e, t;
			return {
				promise: new Promise((n, r) => {
					e = n, t = r;
				}),
				resolve: e,
				reject: t
			};
		}
	}
	let x = (...e) => {
		globalThis?.process?.env?.JCO_DEBUG && console.debug(...e);
	};
	class S {
		#e;
		#t = null;
		#n;
		#r;
		#i;
		#a = null;
		#o = !1;
		#s = null;
		target;
		constructor(e) {
			let { componentIdx: t, target: n } = e;
			this.#e = t, this.target = e.target, this.#c();
		}
		componentIdx() {
			return this.#e;
		}
		isInSet() {
			return this.#a !== null;
		}
		idx() {
			return this.#s;
		}
		setIdx(e) {
			if (e === 0) throw Error("waitable idx cannot be zero");
			this.#s = e;
		}
		setTarget(e) {
			this.target = e;
		}
		#c() {
			let { promise: e, resolve: t, reject: n } = b();
			this.#n = e, this.#r = t, this.#i = n;
		}
		resolve() {
			this.#r();
		}
		reject(e) {
			this.#i(e);
		}
		promise() {
			return this.#n;
		}
		hasPendingEvent() {
			return this.#t !== null;
		}
		setPendingEvent(e) {
			x("[Waitable#setPendingEvent()] args", {
				waitable: this,
				inSet: this.#a
			}), this.#t = e;
		}
		getPendingEvent() {
			if (x("[Waitable#getPendingEvent()] args", {
				waitable: this,
				inSet: this.#a,
				hasPendingEvent: this.#t !== null
			}), this.#t === null) return null;
			let e = this.#t;
			this.#t = null;
			let t = e();
			return this.#c(), t;
		}
		join(e) {
			if (x("[Waitable#join()] args", {
				waitable: this,
				waitableSet: e,
				isRemoval: e === null
			}), this.#a === void 0) throw TypeError("waitable set must be not be undefined");
			this.#a && this.#a.removeWaitable(this), this.#a = e, e && this.#a.addWaitable(this);
		}
		drop() {
			x("[Waitable#drop()] args", {
				componentIdx: this.#e,
				waitable: this
			}), this.join(null);
		}
		async waitForPendingEvent(e) {
			let { cstate: t } = e;
			if (!t) throw TypeError("missing component state");
			if (this.#a !== null || this.#o) throw Error("waitable is already in a set/has a sync waiter");
			this.#o = !0, await t.waitUntil({
				cancellable: !1,
				readyFn: () => this.hasPendingEvent()
			}), this.#o = !1;
		}
	}
	let C = /* @__PURE__ */ new Map(), w = { error: null }, T = {
		deadlockCheck: null,
		pendingHostOperations: 0
	}, E = a.RuntimeError;
	function D() {
		T.deadlockCheck === null && w.error === null && (T.deadlockCheck = setTimeout(() => {
			if (T.deadlockCheck = null, w.error !== null || T.pendingHostOperations > 0) return;
			let e = /* @__PURE__ */ new Set();
			for (let t of y.values()) {
				if (t.hasPendingSchedulerWork()) {
					t.runTickLoop();
					return;
				}
				for (let n of t.suspendedTaskMetas()) e.add(n.task);
			}
			let t = /* @__PURE__ */ new Set();
			for (let n of e) {
				let e = n.getRootTask();
				e.isResolvedState() || t.add(e);
			}
			if (t.size === 0) return;
			let n = new E("wasm trap: deadlock detected: event loop cannot make further progress");
			n.deadlockDetail = {
				pendingHostOperations: T.pendingHostOperations,
				suspendedTasks: [...e].map((e) => ({
					taskID: e.id(),
					componentIdx: e.componentIdx(),
					state: e.taskState(),
					rootTaskID: e.getRootTask().id()
				})),
				unresolvedRootTaskIDs: [...t].map((e) => e.id())
			}, w.error = n;
			for (let e of t) e.setErrored(n), e.reject(n);
			for (let r of e) !r.isResolvedState() && t.has(r.getRootTask()) && (r.setErrored(n), r.reject(n));
			for (let e of y.values()) e.markTrapped(n);
			for (let e of y.values()) e.abandonLockWaiters();
			for (let e of y.values()) e.abandonAdmissionWaiters();
			for (let e of y.values()) e.runTickLoop();
		}, 0));
	}
	let O = /* @__PURE__ */ new Map([
		["unreachable", "wasm trap: wasm `unreachable` instruction executed"],
		["memory access out of bounds", "wasm trap: out of bounds memory access"],
		["divide by zero", "wasm trap: integer divide by zero"],
		["remainder by zero", "wasm trap: integer divide by zero"],
		["divide result unrepresentable", "wasm trap: integer overflow"],
		["float unrepresentable in integer range", "wasm trap: invalid conversion to integer"],
		["table index is out of bounds", "wasm trap: undefined element: out of bounds table access"],
		["function signature mismatch", "wasm trap: indirect call type mismatch"],
		["call stack exhausted", "wasm trap: call stack exhausted"]
	]);
	function k(e) {
		if (!(e instanceof E)) return e;
		let t = O.get(e.message);
		return t !== void 0 && (e.message = t), e;
	}
	class A {
		static FREE = Symbol("RepTable.free");
		#e = [0, null];
		#t = 0;
		#n = null;
		constructor(e) {
			this.target = e?.target, this.#n = e?.space ?? null;
		}
		data() {
			return this.#e;
		}
		insert(e) {
			if (this.#n !== null) {
				let t = this.#n.alloc();
				return this.#e[t << 1] = e, this.#e[(t << 1) + 1] = null, this.#t += 1, t;
			}
			x("[RepTable#insert()] args", {
				val: e,
				target: this.target
			});
			let t = this.#e[0];
			if (t === 0) {
				this.#e.push(e), this.#e.push(null);
				let t = (this.#e.length >> 1) - 1;
				return x("[RepTable#insert()] inserted", {
					val: e,
					target: this.target,
					rep: t
				}), this.#t += 1, t;
			}
			let n = t << 1;
			if (this.#e[n] !== A.FREE) throw Error("corrupt rep table freelist: head does not point at a freed slot");
			return this.#e[0] = this.#e[n + 1], this.#e[n] = e, this.#e[n + 1] = null, x("[RepTable#insert()] inserted", {
				val: e,
				target: this.target,
				rep: t
			}), this.#t += 1, t;
		}
		get(e) {
			if (x("[RepTable#get()] args", {
				rep: e,
				target: this.target
			}), e === 0) throw new E("invalid resource rep during get, (cannot be 0)");
			let t = e << 1, n = this.#e[t];
			if (n !== A.FREE) return n;
		}
		contains(e) {
			if (x("[RepTable#contains()] args", {
				rep: e,
				target: this.target
			}), e === 0) throw Error("invalid resource rep during contains, (cannot be 0)");
			let t = e << 1, n = this.#e[t];
			return n !== A.FREE && !!n;
		}
		remove(e) {
			if (x("[RepTable#remove()] args", {
				rep: e,
				target: this.target
			}), e === 0) throw Error("invalid resource rep during remove, (cannot be 0)");
			if (this.#n !== null) {
				let t = e << 1, n = this.#e[t];
				if (n === void 0 || n === A.FREE) throw new E(`unknown handle index ${e}`);
				return this.#e[t] = A.FREE, this.#e[t + 1] = null, --this.#t, this.#n.release(e), n;
			}
			if (this.#e.length === 2) throw Error("invalid");
			let t = e << 1;
			if (t >= this.#e.length) throw Error(`invalid rep [${e}] during remove, out of range`);
			let n = this.#e[t];
			if (n === A.FREE) throw new E(`double removal of rep [${e}] (already freed)`);
			return this.#e[t] = A.FREE, this.#e[t + 1] = this.#e[0], this.#e[0] = e, --this.#t, n;
		}
		size() {
			return this.#t;
		}
		clear() {
			x("[RepTable#clear()] args", {
				rep,
				target: this.target
			}), this.#e = [0, null];
		}
	}
	class j {
		static EVENT_HANDLER_EVENTS = ["backpressure-change"];
		static TickResult = {
			DONE: "done",
			RESUMED: "resumed",
			IDLE: "idle"
		};
		#e;
		#t = !1;
		#n = b();
		#r = null;
		#i = [];
		#a = [];
		#o = !1;
		#s = 0;
		#c = /* @__PURE__ */ new Map();
		#l = null;
		#u = !1;
		#d = null;
		#f = 0;
		#p = 0n;
		#m = /* @__PURE__ */ new Map();
		#h = 0n;
		#g = null;
		#_ = [];
		#v = !0;
		handles;
		subtasks;
		threads;
		constructor(e) {
			this.#e = e.componentIdx, this.handles = new A({
				target: `component [${this.#e}] handles (waitable objects)`,
				space: g(this.#e)
			}), this.subtasks = new A({ target: `component [${this.#e}] subtasks` }), this.threads = new A({ target: `component [${this.#e}] threads` });
		}
		componentIdx() {
			return this.#e;
		}
		get mayLeave() {
			let e = C.get(this.#e);
			return e === void 0 ? this.#v : e.value === 1;
		}
		set mayLeave(e) {
			if (typeof e != "boolean") throw TypeError("mayLeave must be a boolean");
			this.#v = e;
			let t = C.get(this.#e);
			t !== void 0 && (t.value = +!!e);
		}
		errored() {
			return this.#l !== null;
		}
		setErrored(e) {
			x("[ComponentAsyncState#setErrored()] component errored", {
				err: e,
				componentIdx: this.#e
			}), this.#l ||= (e || (e = /* @__PURE__ */ Error("error elswehere (see other component instance error)"), e.componentIdx = this.#e), e);
		}
		markTrapped(e) {
			return e instanceof E && (e = k(e), this.#u = !0, this.#d === null && (this.#d = e), x("[ComponentAsyncState#markTrapped()] component trapped", {
				err: e,
				componentIdx: this.#e
			}), w.error === null && (w.error = e), !0);
		}
		isTrapped() {
			return this.#u;
		}
		throwIfInterrupted() {
			if (this.#u) throw this.#d ?? new E("wasm trap: cannot enter component instance");
		}
		throwIfTrapped() {
			if (this.#u) throw new E("wasm trap: cannot enter component instance");
		}
		callingSyncImport(e) {
			if (e === void 0) return this.#t;
			if (typeof e != "boolean") throw TypeError("invalid setting for async import");
			let t = this.#t;
			this.#t = e, t === !0 && this.#t === !1 && this.#y();
		}
		#y() {
			let e = this.#n;
			this.#n = b(), e.resolve();
		}
		async waitForSyncImportCallEnd() {
			await this.#n.promise;
		}
		setBackpressure(e) {
			return this.#f = e, this.#f;
		}
		getBackpressure() {
			return this.#f;
		}
		incrementBackpressure() {
			let e = this.#f;
			if (e < 0 || e > 2 ** 16) throw Error(`invalid current backpressure value [${e}]`);
			let t = this.getBackpressure() + 1;
			if (t >= 2 ** 16) throw Error(`invalid new backpressure value [${t}], overflow`);
			return this.setBackpressure(t);
		}
		decrementBackpressure() {
			let e = this.#f;
			if (e < 0 || e > 2 ** 16) throw Error(`invalid current backpressure value [${e}]`);
			let t = e - 1;
			if (t < 0) throw new E("backpressure counter underflow");
			return this.setBackpressure(t);
		}
		hasBackpressure() {
			return this.#f > 0;
		}
		waitForBackpressure() {
			let e = !1, t = this;
			t.addBackpressureWaiter();
			let n = this.registerHandler({
				event: "backpressure-change",
				fn: (r) => {
					r === 0 && (t.removeHandler(n), e = !0);
				}
			});
			return new Promise((n) => {
				let r = setInterval(() => {
					e || (clearInterval(r), t.removeBackpressureWaiter(), n(null));
				}, 0);
			});
		}
		registerHandler(e) {
			let { event: t, fn: n } = e;
			if (!t) throw Error("missing handler event");
			if (!n) throw Error("missing handler fn");
			if (!j.EVENT_HANDLER_EVENTS.includes(t)) throw Error(`unrecognized event handler [${t}]`);
			let r = this.#h++, i = this.#m.get(t);
			return i || (i = [], this.#m.set(t, i)), i.push({
				id: r,
				fn: n,
				event: t
			}), r;
		}
		removeHandler(e) {
			let { event: t, handlerID: n } = e, r = this.#m.get(t);
			r && r.find((e) => e.id === n) && this.#m.set(t, this.#m.get(t).filter((e) => e.id !== n));
		}
		getBackpressureWaiters() {
			return this.#p;
		}
		addBackpressureWaiter() {
			this.#p++;
		}
		removeBackpressureWaiter() {
			if (this.#p--, this.#p < 0) throw Error("unexepctedly negative number of backpressure waiters");
		}
		isExclusivelyLocked() {
			return this.#r !== null;
		}
		exclusivelyLockedBy(e) {
			return this.#r === e;
		}
		exclusiveLock(e) {
			if (x("[ComponentAsyncState#exclusiveLock()]", {
				holder: this.#r,
				requester: e,
				componentIdx: this.#e
			}), e == null) throw Error("exclusive lock requires the acquiring task id");
			if (this.#r !== null) throw Error(`component [${this.#e}] exclusive lock held by task [${this.#r}], requested by [${e}]`);
			this.#r = e;
		}
		acquireExclusiveLock(e) {
			if (e == null) throw Error("exclusive lock requires the acquiring task id");
			if (this.#r === null) this.#r = e, x("[ComponentAsyncState#acquireExclusiveLock()] acquired", {
				holder: e,
				componentIdx: this.#e
			});
			else {
				if (this.#r === e) throw Error(`task [${e}] already holds the lock for component [${this.#e}]`);
				return x("[ComponentAsyncState#acquireExclusiveLock()] waiting", {
					holder: this.#r,
					requester: e,
					componentIdx: this.#e,
					queued: this.#i.length
				}), new Promise((t) => {
					this.#i.push({
						taskID: e,
						resolve: t
					});
				});
			}
		}
		takeAdmissionTicket(e) {
			let t = {
				taskID: e,
				...b()
			};
			return this.#a.push(t), t;
		}
		hasPendingAdmissions() {
			return this.#a.length > 0;
		}
		waitForAdmissionTurn(e) {
			if (this.#a[0] !== e) return e.promise;
		}
		releaseAdmissionTicket(e) {
			let t = this.#a.indexOf(e);
			t !== -1 && (this.#a.splice(t, 1), t === 0 && this.#a.length > 0 && this.#a[0].resolve());
		}
		abandonAdmissionWaiters() {
			let e = this.#a.splice(0);
			for (let t of e) t.resolve();
		}
		abandonLockWaiters() {
			let e = this.#i.splice(0);
			for (let t of e) t.resolve();
		}
		cancelExclusiveLockWaiter(e) {
			let t = this.#i.findIndex((t) => t.taskID === e);
			if (t === -1) return !1;
			let [n] = this.#i.splice(t, 1);
			return n.resolve(), !0;
		}
		exclusiveRelease(e) {
			if (x("[ComponentAsyncState#exclusiveRelease()] args", {
				holder: this.#r,
				releaser: e,
				componentIdx: this.#e
			}), this.#r !== e) return x("[ComponentAsyncState#exclusiveRelease()] ignoring foreign release", {
				holder: this.#r,
				releaser: e,
				componentIdx: this.#e
			}), !1;
			this.#r = null, this.#_ = this.#_.filter((e) => !!e);
			for (let [e, t] of this.#_.entries()) try {
				this.#_[e] = null, t();
			} catch (e) {
				throw x("error while executing handler for next exclusive release", e), e;
			}
			return this.#b(), !0;
		}
		#b() {
			this.#o || this.#i.length === 0 || (this.#o = !0, queueMicrotask(() => {
				if (this.#o = !1, this.#r !== null) return;
				let e = this.#i.shift();
				e && (this.#r = e.taskID, e.resolve());
			}));
		}
		onNextExclusiveRelease(e) {
			x("[ComponentAsyncState#()onNextExclusiveRelease] registering"), this.#_.push(e);
		}
		async waitForExclusiveRelease() {
			for (; this.isExclusivelyLocked();) await new Promise((e) => this.onNextExclusiveRelease(e));
		}
		#x(e) {
			let t = [...this.#c.values()].filter((t) => t.thread.taskID() === e);
			return t.find((e) => e.cancellable && this.suspendedThreadReady(e.thread)) ?? t[0];
		}
		suspendTask(e) {
			let { task: t, ...n } = e;
			return t.endFirstSlice(), this.suspendThread({
				thread: t.defaultThread(),
				...n
			});
		}
		resumeTaskByID(e) {
			let t = this.#x(e);
			return t ? this.resumeThread(t.thread) : !1;
		}
		suspendedTaskReady(e) {
			let t = this.#x(e);
			return t ? this.suspendedThreadReady(t.thread) : !1;
		}
		suspendedTaskCancellable(e) {
			return !!this.#x(e)?.cancellable;
		}
		isTaskSuspended(e) {
			return !!this.#x(e);
		}
		*suspendedTaskMetas() {
			for (let e of this.#c.values()) yield {
				...e,
				task: e.thread.task()
			};
		}
		#S(e) {
			let t = this.#c.get(e);
			return this.#c.delete(e), t;
		}
		suspendThread(e) {
			let { thread: t, readyFn: n, cancellable: r, onResume: i, suspended: a = !1 } = e;
			if (x("[ComponentAsyncState#suspendThread()]", {
				threadID: t.id(),
				taskID: t.taskID(),
				componentIdx: this.#e
			}), t.componentIdx() !== this.#e) throw Error("assert: thread component idx should match async state");
			if (this.#c.has(t)) throw Error(`thread [${t.id()}] already suspended`);
			let o, s;
			if (i) s = () => i(!t.isCancelled() && !t.isRejected() && !this.isTrapped());
			else {
				let e = b();
				o = e.promise, s = () => e.resolve(!t.isCancelled() && !t.isRejected() && !this.isTrapped());
			}
			return this.#c.set(t, {
				thread: t,
				cancellable: r,
				readyFn: n,
				resume: s,
				suspended: a
			}), t.notifyProgress(), this.runTickLoop(), D(), o;
		}
		resumeThread(e) {
			let t = this.#S(e);
			return t ? (t.resume(), !0) : !1;
		}
		markThreadReady(e) {
			let t = this.#c.get(e);
			return !t || !t.suspended ? !1 : (t.suspended = !1, t.readyFn = () => !0, this.runTickLoop(), !0);
		}
		isThreadSuspended(e) {
			return this.#c.get(e)?.suspended ?? !1;
		}
		isThreadReady(e) {
			let t = this.#c.get(e);
			return !!t && !t.suspended && (t.thread.isRejected() || t.readyFn());
		}
		suspendedThreadReady(e) {
			let t = this.#c.get(e);
			if (!t) return !1;
			if (!t.readyFn) throw Error(`suspended thread [${e.id()}] is missing a readiness function`);
			if (this.isTrapped() || t.thread.isRejected()) return !0;
			if (!t.readyFn()) return !1;
			let n = t.thread.task();
			return !n.needsExclusiveLock() || !this.isExclusivelyLocked() || this.exclusivelyLockedBy(n.id());
		}
		addPendingTaskStart() {
			this.#s++;
		}
		removePendingTaskStart() {
			this.#s--;
		}
		hasPendingSchedulerWork() {
			if (this.#s > 0 || this.#o) return !0;
			for (let e of this.#c.values()) if (this.suspendedThreadReady(e.thread)) return !0;
			return !1;
		}
		async runTickLoop() {
			this.#g === null && (this.#g = 1, setTimeout(async () => {
				let e = this.tick();
				for (; e !== j.TickResult.DONE;) e === j.TickResult.IDLE && D(), e === j.TickResult.RESUMED ? await r() : await new Promise((e) => setTimeout(e, 10)), e = this.tick();
				this.#g = null;
			}, 10));
		}
		tick() {
			for (let [e, t] of this.#c) {
				if (!t || !t.readyFn) throw Error(`missing/invalid thread meta despite thread [${e.id()}] being present`);
				if (this.isTrapped() || t.thread.isRejected()) return x("[ComponentAsyncState#tick()] detected thread rejection, leaving early", { meta: t }), this.resumeThread(e), j.TickResult.RESUMED;
				if (this.suspendedThreadReady(e)) return x("[ComponentAsyncState#tick()] resuming thread via tick", {
					threadID: e.id(),
					componentIdx: this.#e
				}), this.resumeThread(e), j.TickResult.RESUMED;
			}
			return this.#c.size > 0 ? j.TickResult.IDLE : j.TickResult.DONE;
		}
		createWaitable(e) {
			return new S({ target: e?.target });
		}
	}
	function M(e, t) {
		if (!y.has(e)) {
			let t = new j({ componentIdx: e });
			y.set(e, t);
		}
		return y.get(e);
	}
	function N(e, t, n) {
		if (e === -1 || t == null || t === 0) return;
		let r = M(e).threads.get(t);
		if (r && (n === void 0 || r.taskID() === n)) return {
			id: t,
			componentIdx: e,
			taskID: r.taskID(),
			task: r.task(),
			thread: r
		};
	}
	let P = /* @__PURE__ */ new Map(), F = [], ee = [];
	function I(e, t, n = {}) {
		if (x("[clearCurrentThread()] args", {
			componentIdx: e,
			taskID: t
		}), e == null) throw Error("missing/invalid component instance index while ending current task");
		let r = P.get(e);
		if (!r || !Array.isArray(r)) throw Error("missing/invalid tasks for component instance while ending task");
		if (r.length == 0) throw Error(`no current tasks for component instance [${e}] while ending task`);
		let i = t === void 0 ? r.length - 1 : r.findIndex((e) => e.id === t);
		if (i === -1) return;
		let a = r[i];
		n.preserveTask || r.splice(i, 1);
		let o = F.lastIndexOf(a.id);
		return o !== -1 && (F.splice(o, 1), ee.splice(o, 1)), a.task;
	}
	let L = Symbol.dispose || Symbol.for("dispose");
	function R(e, t) {
		if (!t) {
			if (e === null || typeof e != "object" && typeof e != "function") return;
			if (typeof Symbol.asyncDispose == "symbol" && typeof e[Symbol.asyncDispose] == "function") t = e[Symbol.asyncDispose];
			else if (typeof e[L] == "function") t = e[L];
			else return;
		}
		if (i?.walz?.disposal) {
			i.walz.disposal(() => t.call(e));
			return;
		}
		let n = (e) => console.error("[jco] error while disposing discarded host value", e);
		queueMicrotask(() => {
			try {
				let r = t.call(e);
				r && typeof r.then == "function" && r.then(void 0, n);
			} catch (e) {
				n(e);
			}
		});
	}
	let te = /* @__PURE__ */ new Map();
	function z(e) {
		let { componentIdx: t } = e ?? {};
		if (e.componentIdx === void 0) throw TypeError("missing component idx");
		let n = te.get(t);
		return n ? e.memoryIdx === void 0 ? Object.values(n) : n[e.memoryIdx]?.memory : [];
	}
	class B {
		static _ID = 0n;
		static State = {
			STARTING: 0,
			STARTED: 1,
			RETURNED: 2,
			CANCELLED_BEFORE_STARTED: 3,
			CANCELLED_BEFORE_RETURNED: 4
		};
		#e;
		#t = B.State.STARTING;
		#n;
		#r;
		#i = null;
		#a = !1;
		#o = !1;
		#s = null;
		#c = null;
		#l = null;
		#u = null;
		#d = null;
		#f = null;
		#p = null;
		#m = {};
		#h = !1;
		#g = [];
		#_ = [];
		#v = null;
		#y = !1;
		#b = null;
		fnName;
		target;
		isAsync;
		isManualAsync;
		cancelProgress = null;
		constructor(e) {
			if (typeof e.componentIdx != "number") throw Error("invalid componentIdx for subtask creation");
			if (this.#n = e.componentIdx, this.#e = ++B._ID, this.fnName = e.fnName, !e.parentTask) throw Error("missing parent task during subtask creation");
			if (this.#r = e.parentTask, e.childTask && (this.#i = e.childTask), e.memoryIdx && (this.#s = e.memoryIdx), !e.waitable) throw Error("missing/invalid waitable");
			this.#l = e.waitable, e.callMetadata && (this.#m = e.callMetadata), this.#c = [], this.target = e.target, this.isAsync = e.isAsync, this.isManualAsync = e.isManualAsync;
		}
		id() {
			return this.#e;
		}
		parentTaskID() {
			return this.#r?.id();
		}
		childTaskID() {
			return this.#i?.id();
		}
		state() {
			return this.#t;
		}
		getWaitable() {
			return this.#l;
		}
		waitableRep() {
			return this.#l.idx();
		}
		ensureHandle() {
			let e = this.#l.idx();
			if (e != null) return e;
			let t = this.#S().handles.insert(this);
			return this.#l.setIdx(t), this.#l.setTarget(`waitable for subtask (waitable id [${t}], subtask internal ID [${this.id()}])`), t;
		}
		join() {
			return this.#l.join(...arguments);
		}
		getPendingEvent() {
			return this.#l.getPendingEvent(...arguments);
		}
		hasPendingEvent() {
			return this.#l.hasPendingEvent(...arguments);
		}
		setPendingEvent() {
			return this.#l.setPendingEvent(...arguments);
		}
		setTarget(e) {
			this.target = e;
		}
		getResult() {
			if (!this.#y) throw Error("subtask result has not been set");
			return this.#v;
		}
		setResult(e) {
			if (this.#y) throw Error("subtask result has already been set");
			this.#v = e, this.#y = !0;
		}
		componentIdx() {
			return this.#n;
		}
		setChildTask(e) {
			if (!e) throw Error("cannot set missing/invalid child task on subtask");
			if (this.#i) throw Error("child task is already set on subtask");
			if (this.#r === e) throw Error("parent cannot be child");
			this.#i = e;
		}
		getChildTask(e) {
			return this.#i;
		}
		getParentTask() {
			return this.#r;
		}
		setCallbackFn(e, t) {
			if (e) {
				if (this.#u) throw Error("callback fn can only be set once");
				this.#u = e, this.#d = t;
			}
		}
		getCallbackFnName() {
			if (this.#u) return this.#u.name;
		}
		setPostReturnFn(e) {
			if (e) {
				if (this.#f) throw Error("postReturn fn can only be set once");
				this.#f = e;
			}
		}
		setOnProgressFn(e) {
			if (this.#p) throw Error("on progress fn can only be set once");
			this.#p = e;
		}
		isNotStarted() {
			return this.#t == B.State.STARTING;
		}
		cancellationRequested() {
			return this.#o;
		}
		requestCancellation() {
			if (x("[AsyncSubtask#requestCancellation()] args", {
				componentIdx: this.#n,
				subtaskID: this.#e,
				state: this.#t,
				childTaskID: this.childTaskID(),
				fnName: this.fnName
			}), this.#o) throw Error("cancellation has already been requested for this subtask");
			this.#o = !0, this.#h || (this.#i ? this.#i.requestCancellation() : (this.onResolve(null), this.#x()));
		}
		setHostPendingResult(e) {
			e === null || typeof e != "object" && typeof e != "function" || typeof e.then != "function" || (this.#b = e, this.#h && this.#t !== B.State.RETURNED && this.#x());
		}
		#x() {
			let e = this.#b;
			e && (this.#b = null, R(e));
		}
		registerOnStartHandler(e) {
			this.#_.push(e);
		}
		onStart(e) {
			x("[AsyncSubtask#onStart()] args", {
				componentIdx: this.#n,
				subtaskID: this.#e,
				parentTaskID: this.parentTaskID(),
				fnName: this.fnName,
				args: e
			}), this.#p && this.#p(), this.#r.notifyProgress(), this.#t = B.State.STARTED;
			let t;
			return this.#m.startFn && (t = this.#m.startFn.apply(null, e?.startFnParams ?? [])), t;
		}
		registerOnResolveHandler(e) {
			this.#g.push(e);
		}
		reject(e) {
			if (!this.#h) {
				if (this.#p && this.#p(), this.#t === B.State.STARTING) this.#t = B.State.CANCELLED_BEFORE_STARTED;
				else if (this.#t === B.State.STARTED) this.#t = B.State.CANCELLED_BEFORE_RETURNED;
				else throw Error("cannot reject a completed subtask");
				this.#h = !0, this.#b = null, this.#r.removeSubtask(this), this.#r.reject(e);
			}
		}
		onResolve(e) {
			if (x("[AsyncSubtask#onResolve()] args", {
				componentIdx: this.#n,
				subtaskID: this.#e,
				isAsync: this.isAsync,
				childTaskID: this.childTaskID(),
				parentTaskID: this.parentTaskID(),
				parentTaskFnName: this.#r?.entryFnName(),
				fnName: this.fnName
			}), this.#h) throw Error("subtask has already been resolved");
			if (this.#p && this.#p(), e === null && this.#o) {
				if (this.#t === B.State.STARTING) this.#t = B.State.CANCELLED_BEFORE_STARTED;
				else {
					if (this.#t !== B.State.STARTED) throw Error("resolved subtask must have been started before cancellation");
					this.#t = B.State.CANCELLED_BEFORE_RETURNED;
				}
			} else {
				if (this.#t !== B.State.STARTED) throw Error("resolved subtask must have been started before completion");
				this.#t = B.State.RETURNED, this.#b = null;
			}
			this.setResult(e);
			for (let t of this.#g) try {
				t(e);
			} catch (e) {
				throw console.error("error during subtask resolve handler", e), e;
			}
			let t = this.getCallMetadata(), n = t.memory ?? this.#r?.getReturnMemory() ?? z({ componentIdx: this.#r?.componentIdx() })[0];
			if (this.#t === B.State.RETURNED && t && !t.returnFn && (this.isAsync || t.funcTypeIsAsync) && t.resultPtr && n) {
				let { resultPtr: r, realloc: i } = t, a = t.lowers;
				a && a.length > 0 && a[0]({
					componentIdx: this.#n,
					memory: n,
					realloc: i,
					vals: [e],
					storagePtr: r,
					stringEncoding: t.stringEncoding
				});
			}
			if (this.#h = !0, this.#r.removeSubtask(this), !this.isAsync) {
				this.deliverResolve();
				let e = this.waitableRep();
				if (e) try {
					if (this.#S().handles.remove(e) !== this) throw Error("unexpectedly received non-self Subtask from handle removal");
					this.drop();
				} catch (e) {
					x("[AsyncSubtask#onResolve()] failed to remove subtask after sync subtask completion", e);
				}
			}
		}
		getStateNumber() {
			return this.#t;
		}
		isReturned() {
			return this.#t === B.State.RETURNED;
		}
		getCallMetadata() {
			return this.#m;
		}
		isResolved() {
			if (this.#t === B.State.STARTING || this.#t === B.State.STARTED) return !1;
			if (this.#t === B.State.RETURNED || this.#t === B.State.CANCELLED_BEFORE_STARTED || this.#t === B.State.CANCELLED_BEFORE_RETURNED) return !0;
			throw Error("unrecognized internal Subtask state [" + this.#t + "]");
		}
		addLender(e) {
			if (x("[AsyncSubtask#addLender()] args", { handle: e }), !Number.isNumber(e)) throw Error("missing/invalid lender handle [" + e + "]");
			if (this.#c.length === 0 || this.isResolved()) throw Error("subtask has no lendors or has already been resolved");
			e.lends++, this.#c.push(e);
		}
		deliverResolve() {
			if (x("[AsyncSubtask#deliverResolve()] args", {
				lenders: this.#c,
				parentTaskID: this.parentTaskID(),
				subtaskID: this.#e,
				childTaskID: this.childTaskID(),
				resolved: this.isResolved(),
				resolveDelivered: this.resolveDelivered()
			}), this.resolveDelivered() || !this.isResolved()) throw Error("subtask cannot deliver resolution twice, and the subtask must be resolved");
			for (let e of this.#c) e.lends--;
			this.#c = null;
		}
		resolveDelivered() {
			if (x("[AsyncSubtask#resolveDelivered()] args", {}), this.#c === null && !this.isResolved()) throw Error("invalid subtask state, lenders missing and subtask has not been resolved");
			return this.#c === null;
		}
		drop() {
			if (x("[AsyncSubtask#drop()] args", {
				componentIdx: this.#n,
				parentTaskID: this.#r?.id(),
				parentTaskFnName: this.#r?.entryFnName(),
				childTaskID: this.#i?.id(),
				childTaskFnName: this.#i?.entryFnName(),
				subtaskFnName: this.fnName
			}), !this.#l) throw Error("missing/invalid inner waitable");
			if (!this.resolveDelivered()) throw new E("cannot drop a subtask which has not yet resolved");
			this.#l && this.#l.drop(), this.#a = !0;
		}
		#S() {
			let e = M(this.#n);
			if (!e) throw Error("invalid/missing async state for component [" + componentIdx + "]");
			return e;
		}
		getWaitableHandleIdx() {
			if (x("[AsyncSubtask#getWaitableHandleIdx()] args", {}), !this.#l) throw Error("missing/invalid waitable");
			return this.waitableRep();
		}
	}
	class V {
		#e;
		#t;
		#n = 0;
		#r;
		constructor(e) {
			if (typeof e != "function") throw TypeError("future start operation must be a function");
			this.#e = e, this.#r = this.#o.bind(this);
		}
		get then() {
			return this.#n === 0 ? this.#r : void 0;
		}
		#i() {
			return this.#t ||= Promise.resolve().then(this.#e), this.#t;
		}
		resolveAsValue(e) {
			this.#n++;
			try {
				e(this);
			} finally {
				this.#n--;
			}
		}
		#a(e, t) {
			t instanceof V ? t.resolveAsValue(e) : e(t);
		}
		#o(e, t) {
			return this.#i().then((t) => this.#a(e, t.value), t);
		}
	}
	function H(e, t) {
		if (!(t instanceof E)) {
			let e = /call stack|stack overflow|too much recursion/i.test(t?.message ?? "") ? "wasm trap: call stack exhausted" : "wasm trap: uncaught exception propagated out of component";
			t = new E(e, { cause: t });
		}
		return M(e).markTrapped(t), t;
	}
	let U = {};
	function W(e, t, n = !1) {
		if (n && t instanceof RangeError && (t = new E("out of bounds memory access")), t instanceof E) {
			let n = e ?? U.current?.componentIdx;
			n != null && M(n).markTrapped(t);
		}
		return t;
	}
	let G = () => Math.random() > .5, K = {
		NONE: 0,
		SUBTASK: 1,
		STREAM_READ: 2,
		STREAM_WRITE: 3,
		FUTURE_READ: 4,
		FUTURE_WRITE: 5,
		TASK_CANCELLED: 6
	};
	function q(e) {
		let t = e == null ? U.current : U[e];
		if (t != null) return { ...t };
	}
	function J(e) {
		if (x("[_withGlobalCurrentThreadMeta()] args", e), !e) throw TypeError("args missing");
		if (e.taskID === void 0) throw TypeError("missing task ID");
		if (e.threadID === void 0) throw TypeError("missing thread ID");
		if (e.componentIdx === void 0) throw TypeError("missing component idx");
		if (!e.fn) throw TypeError("missing fn");
		let { taskID: t, threadID: n, componentIdx: r, fn: i } = e, a = U[r] ?? null, o = U.current ?? null;
		try {
			return U.current = U[r] = {
				threadID: n,
				taskID: t,
				componentIdx: r
			}, i();
		} catch (t) {
			throw x("error while executing sync callee/callback", {
				...e,
				err: t
			}), t;
		} finally {
			U[r] = a, U.current = o;
		}
	}
	function Y(e) {
		if (x("[_withGlobalCurrentThreadMetaAsync()] args", e), !e) throw TypeError("args missing");
		if (e.taskID === void 0) throw TypeError("missing task ID");
		if (e.threadID === void 0) throw TypeError("missing thread ID");
		if (e.componentIdx === void 0) throw TypeError("missing component idx");
		if (!e.fn) throw TypeError("missing fn");
		let { taskID: t, threadID: n, componentIdx: r, fn: i } = e, a = U[r] ?? null, o = U.current ?? null, s;
		try {
			U.current = U[r] = {
				taskID: t,
				threadID: n,
				componentIdx: r
			}, s = i();
		} catch (t) {
			throw x("error while executing sync callee/callback", {
				...e,
				err: t
			}), U[r] = a, U.current = o, t;
		}
		return s === null || typeof s != "object" && typeof s != "function" || typeof s.then != "function" ? (U[r] = a, U.current = o, s) : (async () => {
			try {
				return await s;
			} catch (t) {
				throw x("error while executing async callee/callback", {
					...e,
					err: t
				}), t;
			} finally {
				U[r]?.threadID === n && U[r]?.taskID === t && (U[r] = null), U.current?.threadID === n && U.current?.taskID === t && (U.current = null);
			}
		})();
	}
	class X {
		static _ID = 0n;
		static _HOST_THREAD_ID = 0;
		static State = {
			INITIAL: "initial",
			CANCELLED: "cancelled",
			CANCEL_PENDING: "cancel-pending",
			CANCEL_DELIVERED: "cancel-delivered",
			RESOLVED: "resolved"
		};
		static BlockResult = {
			CANCELLED: "block.cancelled",
			NOT_CANCELLED: "block.not-cancelled"
		};
		#e;
		#t;
		#n;
		#r;
		#i;
		#a = !0;
		#o = !1;
		#s;
		#c = null;
		#l = [];
		#u = [];
		#d = null;
		#f;
		#p = !1;
		#m;
		#h = !1;
		#g = null;
		#_ = [];
		#v = null;
		#y = null;
		#b = null;
		#x = null;
		#S = null;
		#C = null;
		#w = null;
		#T = null;
		#E = !1;
		#D = null;
		#O = null;
		#k = null;
		#A;
		#j = null;
		#M;
		#N = 0;
		#P = [];
		#F = !1;
		#I = [];
		#L = [];
		#R = !1;
		#z = !1;
		#B = null;
		cancelled = !1;
		cancelRequested = !1;
		alwaysTaskReturn = !1;
		returnCalls = 0;
		tmpRetI64HighBits = 0;
		constructor(e) {
			if (this.#e = ++X._ID, this.#M = ++_, v.set(this.#M, this), e?.componentIdx === void 0) throw TypeError("missing component id during task creation");
			this.#t = e.componentIdx, this.#n = X.State.INITIAL, this.#r = e?.isAsync ?? !1, this.#i = e?.isManualAsync ?? !1, this.#s = e?.preserveFutureResult ?? !1, this.#c = e.entryFnName, this.#a = e?.callingWasmExport !== !1;
			let { promise: t, resolve: n, reject: r } = b();
			this.#d = t, t.catch(() => {});
			let i = !1, a = () => {
				!i && this.#p && (i = !0, this.#B === null ? this.#h ? r(this.#f) : this.#s && this.#f instanceof V ? this.#f.resolveAsValue(n) : n(this.#f) : r(this.#B));
			};
			this.#m = a, this.#l.push((e) => {
				this.#k === null && (this.#r || this.#i) && (this.#f = e, this.#p = !0);
			});
			let { promise: o, resolve: s, reject: c } = b();
			this.#g = o, this.#_.push(() => {
				this.#k === null && (this.#r || this.#i) && a(), s();
			}), e.callbackFn && (this.#b = e.callbackFn), e.callbackFnName && (this.#x = e.callbackFnName), e.getCalleeParamsFn && (this.#C = e.getCalleeParamsFn), e.stringEncoding && (this.#O = e.stringEncoding), e.parentSubtask && (this.#k = e.parentSubtask), e.errHandling && (this.#A = e.errHandling);
		}
		taskState() {
			return this.#n;
		}
		id() {
			return this.#e;
		}
		componentIdx() {
			return this.#t;
		}
		entryFnName() {
			return this.#c;
		}
		resourceScopeId() {
			return this.#M;
		}
		addBorrowedHandle() {
			if (this.#F) throw Error("cannot add a borrow to an exited resource scope");
			this.#N++;
		}
		removeBorrowedHandle() {
			if (this.#N === 0) throw Error("resource borrow count underflow");
			this.#N--;
		}
		addResourceLender(e, t) {
			if (this.#F) throw Error("cannot add a lender to an exited resource scope");
			this.#P.push({
				table: e,
				handle: t
			});
		}
		validateResourceBorrowScope() {
			if (!this.#F) {
				if (this.#N !== 0) throw new E("borrow handles still remain at the end of the call");
				for (let { table: e, handle: t } of this.#P) {
					let n = t << 1, r = e[n];
					if (!Number.isInteger(r) || r <= 0 || r >= 2 ** 30) throw Error("invalid resource lender state at scope exit");
					e[n] = r - 1;
				}
				this.#P = [], this.#F = !0, v.delete(this.#M);
			}
		}
		completionPromise() {
			return this.#d;
		}
		settleCompletion() {
			this.#m();
		}
		exitPromise() {
			return this.#g;
		}
		waitForProgress() {
			let { promise: e, resolve: t } = b();
			return this.#u.push(t), e;
		}
		notifyProgress() {
			let e = this.#u;
			this.#u = [];
			for (let t of e) t();
		}
		isAsync() {
			return this.#r;
		}
		isManualAsync() {
			return this.#i;
		}
		isSync() {
			return !this.isAsync();
		}
		getErrHandling() {
			return this.#A;
		}
		hasCallback() {
			return this.#b !== null;
		}
		getReturnMemoryIdx() {
			return this.#v;
		}
		setReturnMemoryIdx(e) {
			e !== null && (this.#v = e);
		}
		getReturnMemory() {
			return this.#y;
		}
		setReturnMemory(e) {
			e !== null && (this.#y = e);
		}
		setReturnLowerFns(e) {
			this.#j = e;
		}
		getReturnLowerFns() {
			return this.#j;
		}
		calleeIsAsync() {
			return this.#w === !0;
		}
		setCalleeIsAsync(e) {
			if (typeof e != "boolean") throw TypeError("callee async state must be a boolean");
			this.#w = e;
		}
		setParentSubtask(e) {
			if (e && e instanceof B) {
				if (this.#k) throw Error("parent subtask can only be set once");
				this.#k = e;
			}
		}
		getParentSubtask() {
			return this.#k;
		}
		addThread(e) {
			if (this.#t === -1) return e.setId(++X._HOST_THREAD_ID), this.#L.push(e), e;
			let t = M(this.#t);
			return e.setId(t.threads.insert(e)), this.#L.push(e), e;
		}
		#V() {
			if (this.#L.length === 1 && !this.isResolvedState() && !this.isRejected()) {
				let e = new E(`(component [${this.#t}]) task [${this.#e}] exited without resolution`);
				throw this.#t === -1 ? e : W(this.#t, e);
			}
		}
		removeThread(e) {
			this.#V(), this.settleCompletion(), this.#t !== -1 && M(this.#t).threads.remove(e.id()), this.#L = this.#L.filter((t) => t !== e), this.#z && this.#L.length === 0 && I(this.#t, this.#e);
		}
		defaultThread() {
			return this.#L[0];
		}
		getRootTask() {
			let e = this.getParentSubtask(), t = this;
			for (; e;) t = e.getParentTask(), e = t.getParentSubtask();
			return t;
		}
		setPostReturnFn(e) {
			if (e) {
				if (this.#S) throw Error("postReturn fn can only be set once");
				this.#S = e;
			}
		}
		setCallbackFn(e, t) {
			if (e) {
				if (this.#b) throw Error("callback fn can only be set once");
				this.#b = e, this.#x = t;
			}
		}
		getCallbackFnName() {
			if (this.#x) return this.#x;
		}
		runCallbackFn(...e) {
			if (!this.#b) throw Error("no callback function has been set for task");
			return this.#b._jcoMaySuspend === !1 ? J({
				taskID: this.#e,
				threadID: this.defaultThread().id(),
				componentIdx: this.#t,
				fn: () => this.#b.apply(null, e)
			}) : Y({
				taskID: this.#e,
				threadID: this.defaultThread().id(),
				componentIdx: this.#t,
				fn: () => this.#b.apply(null, e)
			});
		}
		getCalleeParams() {
			if (!this.#C) throw Error("missing/invalid getCalleeParamsFn");
			return this.#C();
		}
		mayBlock() {
			return this.isAsync() || this.isManualAsync() || this.isResolvedState() || this.#E;
		}
		setFuncTypeIsAsync(e) {
			this.#E = e;
		}
		setReturnResultTypeIdx(e) {
			this.#D = e;
		}
		returnResultTypeIdx() {
			return this.#D;
		}
		mayEnter(e) {
			let t = M(this.#t);
			return t.hasBackpressure() ? (x("[AsyncTask#mayEnter()] disallowed due to backpressure", { taskID: this.#e }), !1) : t.callingSyncImport() ? !t.callingSyncExport || e.isAsync ? (x("[AsyncTask#mayEnter()] disallowed due to sync export w/ sync pending", { taskID: this.#e }), !1) : !0 : (x("[AsyncTask#mayEnter()] disallowed due to sync import call", { taskID: this.#e }), !1);
		}
		enterSync() {
			if (this.needsExclusiveLock()) {
				let e = M(this.#t);
				e.isExclusivelyLocked() ? (this.#o = !0, x("[AsyncTask#enterSync()] entering without exclusive lock", {
					taskID: this.#e,
					componentIdx: this.#t
				})) : e.exclusiveLock(this.#e);
			}
			return !0;
		}
		tryEnter() {
			if (this.#R) throw Error(`task with ID [${this.#e}] should not be entered twice`);
			if (this.deliverPendingCancel({ cancellable: !0 })) return this.cancel(), !1;
			let e = M(this.#t);
			if (this.isSync()) return this.#R = !0, !0;
			if (e.hasBackpressure() || e.hasPendingAdmissions()) return null;
			if (this.needsExclusiveLock()) {
				if (e.isExclusivelyLocked()) return null;
				e.exclusiveLock(this.#e);
			}
			return this.deliverPendingCancel({ cancellable: !0 }) ? (e.exclusiveRelease(this.#e), this.cancel(), !1) : (this.#R = !0, !0);
		}
		enterHost() {
			if (this.#R) throw Error(`task with ID [${this.#e}] should not be entered twice`);
			if (this.deliverPendingCancel({ cancellable: !0 })) return this.cancel(), !1;
			M(this.#t), this.#R = !0;
			let e = this.#k?.getParentTask();
			return (e?.taskState() === X.State.CANCEL_DELIVERED || e && !e.hasCallback()) && e.notifyProgress(), this.#R;
		}
		async enter(e) {
			if (x("[AsyncTask#enter()] args", {
				taskID: this.#e,
				componentIdx: this.#t,
				subtaskID: this.getParentSubtask()?.id(),
				args: e,
				entryFnName: this.#c
			}), e?.isHost) return this.enterHost();
			if (this.#R) throw Error(`task with ID [${this.#e}] should not be entered twice`);
			if (this.deliverPendingCancel({ cancellable: !0 })) return this.cancel(), !1;
			let t = M(this.#t);
			if (this.isSync()) return this.#R = !0, this.#i && this.needsExclusiveLock() && await t.acquireExclusiveLock(this.#e), this.#R;
			let n = t.takeAdmissionTicket(this.#e);
			this.#T = n;
			let r = !1;
			try {
				return r = await this.#H(t, n), r;
			} finally {
				r || this.endFirstSlice();
			}
		}
		endFirstSlice() {
			let e = this.#T;
			e !== null && (this.#T = null, M(this.#t).releaseAdmissionTicket(e));
		}
		async #H(e, t) {
			if (await e.waitForAdmissionTurn(t), this.isResolvedState() || this.isCancelled()) return !1;
			for (;;) {
				if (e.hasBackpressure()) {
					e.addBackpressureWaiter();
					let t = await this.waitUntil({
						readyFn: () => !e.hasBackpressure(),
						cancellable: !0
					});
					if (e.removeBackpressureWaiter(), !t || this.isCancelled()) return this.isResolvedState() || this.cancel(), !1;
				}
				if (this.needsExclusiveLock() && (await e.acquireExclusiveLock(this.#e), e.hasBackpressure())) e.exclusiveRelease(this.#e);
				else break;
			}
			try {
				e.throwIfInterrupted();
			} catch (t) {
				return e.exclusiveRelease(this.#e), this.setErrored(t), !1;
			}
			return this.isResolvedState() || this.isCancelled() ? (e.exclusiveRelease(this.#e), !1) : this.deliverPendingCancel({ cancellable: !0 }) ? (e.exclusiveRelease(this.#e), this.cancel(), !1) : (this.#R = !0, this.#R);
		}
		isRunningState() {
			return this.#n !== X.State.RESOLVED;
		}
		isResolvedState() {
			return this.#n === X.State.RESOLVED;
		}
		isResolved() {
			return this.#n === X.State.RESOLVED;
		}
		isExited() {
			return this.#z;
		}
		async waitUntil(e) {
			let { readyFn: t, cancellable: n } = e;
			return x("[AsyncTask#waitUntil()] args", {
				taskID: this.#e,
				args: { cancellable: n }
			}), await this.suspendUntil({
				readyFn: t,
				cancellable: n
			});
		}
		async yieldUntil(e) {
			let { readyFn: t, cancellable: n } = e;
			return x("[AsyncTask#yieldUntil()]", {
				taskID: this.#e,
				args: { cancellable: n },
				componentIdx: this.#t
			}), await this.immediateSuspend({
				readyFn: t,
				cancellable: n
			}) ? {
				code: K.NONE,
				payload0: 0,
				payload1: 0
			} : {
				code: K.TASK_CANCELLED,
				payload0: 0,
				payload1: 0
			};
		}
		async suspendUntil(e) {
			let { cancellable: t, readyFn: n } = e;
			return x("[AsyncTask#suspendUntil()] args", {
				taskID: this.#e,
				args: { cancellable: t },
				componentIdx: this.#t
			}), !this.deliverPendingCancel({ cancellable: t }) && await this.immediateSuspendUntil({
				readyFn: n,
				cancellable: t
			});
		}
		suspendUntilCallback(e, t) {
			let { cancellable: n, readyFn: r } = e;
			this.deliverPendingCancel({ cancellable: n }) ? t(!1) : M(this.#t).suspendTask({
				task: this,
				cancellable: n,
				readyFn: () => n && this.#n === X.State.CANCEL_PENDING ? !0 : r(),
				onResume: (e) => {
					e && this.deliverPendingCancel({ cancellable: n }) && (e = !1), t(e);
				}
			});
		}
		async immediateSuspendUntil(e) {
			let { cancellable: t, readyFn: n } = e;
			return x("[AsyncTask#immediateSuspendUntil()] args", {
				args: {
					cancellable: t,
					readyFn: n
				},
				taskID: this.#e,
				componentIdx: this.#t
			}), n() && G() ? !0 : await this.immediateSuspend({
				cancellable: t,
				readyFn: n
			});
		}
		async immediateSuspend(e) {
			let t = M(this.#t), n = this.#R ? t.threads.get(q(this.#t).threadID) : this.defaultThread();
			if (!n || n.task() !== this) throw Error("invalid current thread for wait");
			return n.suspend({
				...e,
				releaseLock: !1
			});
		}
		deliverPendingCancel(e) {
			let { cancellable: t } = e;
			return x("[AsyncTask#deliverPendingCancel()]", {
				args: { cancellable: t },
				taskID: this.#e,
				componentIdx: this.#t
			}), t && this.#n === X.State.CANCEL_PENDING ? (this.#n = X.State.CANCEL_DELIVERED, !0) : !1;
		}
		isCancelled() {
			return this.cancelled;
		}
		cancellationRequested() {
			return this.cancelRequested;
		}
		cancellationPending() {
			return this.#n === X.State.CANCEL_PENDING;
		}
		requestCancellation() {
			if (x("[AsyncTask#requestCancellation()] args", {
				taskID: this.#e,
				componentIdx: this.#t,
				state: this.#n
			}), !(this.isResolvedState() || this.cancelRequested)) {
				if (this.cancelRequested = !0, this.#n === X.State.INITIAL && (this.#n = X.State.CANCEL_PENDING, !this.#R)) {
					this.deliverPendingCancel({ cancellable: !0 }), this.cancel();
					let e = M(this.#t);
					e.resumeTaskByID(this.#e), e.cancelExclusiveLockWaiter(this.#e), this.exit({ skipExclusiveLockCheck: !0 });
				} else M(this.#t).runTickLoop();
			}
		}
		cancel(e) {
			if (x("[AsyncTask#cancel()] args", {}), this.taskState() !== X.State.CANCEL_DELIVERED) throw Error("`task.cancel` called by task which has not been cancelled");
			this.validateResourceBorrowScope(), this.cancelled = !0, this.onResolve(e?.error ?? null), this.#n = X.State.RESOLVED, this.#R || this.notifyProgress();
		}
		onResolve(e) {
			let t = this.#l;
			this.#l = [];
			for (let n of t) try {
				n(e);
			} catch (e) {
				throw x("[AsyncTask#onResolve] error during task resolve handler", e), e;
			}
			if (this.#h) {
				this.#k?.reject(e);
				return;
			}
			let n = this.#k && !this.#k.isResolved(), r = !this.isCancelled();
			if (n && r) {
				let t = this.#k.getCallMetadata();
				if (t.returnFn && !t.returnFnCalled) {
					x("[AsyncTask#onResolve()] running returnFn", {
						componentIdx: this.#t,
						taskID: this.#e,
						subtaskID: this.#k.id()
					});
					let n = this.#k.getParentTask();
					J({
						taskID: n.id(),
						threadID: t.callerThreadID,
						componentIdx: n.componentIdx(),
						fn: () => t.returnFn.apply(null, [e, t.resultPtr])
					}), t.returnFnCalled = !0;
				}
			}
			if (this.#S && r) {
				x("[AsyncTask#onResolve()] running post return ", {
					componentIdx: this.#t,
					taskID: this.#e
				});
				try {
					J({
						taskID: this.#e,
						threadID: this.defaultThread().id(),
						componentIdx: this.#t,
						fn: () => this.#S(e)
					});
				} catch (e) {
					throw e = H(this.#t, e), this.reject(e), e;
				}
			}
			n && this.#k.onResolve(e);
		}
		registerOnResolveHandler(e) {
			this.#l.push(e);
		}
		isRejected() {
			return this.#h;
		}
		isErrored() {
			return this.#B;
		}
		setErrored(e) {
			this.#B === null && (this.#B = e);
		}
		reject(e) {
			if (x("[AsyncTask#reject()] args", {
				componentIdx: this.#t,
				taskID: this.#e,
				parentSubtask: this.#k,
				parentSubtaskID: this.#k?.id(),
				entryFnName: this.entryFnName(),
				callbackFnName: this.#x,
				errMsg: e.message
			}), this.setErrored(e), !this.#h) {
				if (this.isResolvedState()) {
					this.#h = !0, this.#B = e;
					let t = this.#k?.getParentTask();
					t && t.reject(e);
				} else this.#h = !0, this.cancelRequested = !0, this.#n = X.State.CANCEL_PENDING, this.deliverPendingCancel({ cancellable: !0 }), this.cancel({ error: e });
			}
		}
		resolve(e) {
			if (x("[AsyncTask#resolve()] args", {
				componentIdx: this.#t,
				taskID: this.#e,
				entryFnName: this.entryFnName(),
				callbackFnName: this.#x
			}), this.#n === X.State.RESOLVED) throw this.#B === null ? Error(`(component [${this.#t}]) task [${this.#e}]  is already resolved (did you forget to wait for an import?)`) : this.#B;
			switch (this.validateResourceBorrowScope(), this.#n = X.State.RESOLVED, e.length) {
				case 0:
					this.onResolve(void 0);
					break;
				case 1:
					this.onResolve(e[0]);
					break;
				default: throw x("[AsyncTask#resolve()] unexpected number of results", {
					componentIdx: this.#t,
					results: e,
					taskID: this.#e,
					subtaskID: this.#k?.id(),
					entryFnName: this.#c,
					callbackFnName: this.#x
				}), Error("unexpected number of results");
			}
		}
		exit(e) {
			if (x("[AsyncTask#exit()]", {
				componentIdx: this.#t,
				taskID: this.#e
			}), this.endFirstSlice(), this.#z) throw Error("task has already exited");
			this.#V(), this.validateResourceBorrowScope();
			let t = this.#t === -1 ? null : M(this.#t);
			if (!t && this.#t !== -1) throw Error("missing async state for component [" + this.#t + "]");
			if (t && !e?.skipExclusiveLockCheck && !this.#o && this.needsExclusiveLock() && !t.exclusivelyLockedBy(this.#e)) throw Error(`task [${this.#e}] exit: component [${this.#t}] should have been exclusively locked by it`);
			t && t.exclusiveRelease(this.#e), this.notifyProgress();
			for (let e of this.#_) try {
				e();
			} catch (e) {
				throw console.error("error during task exit handler", e), e;
			}
			this.#z = !0, this.removeThread(this.defaultThread()), this.#L.length > 0 && I(this.#t, this.#e, { preserveTask: !0 });
		}
		needsExclusiveLock() {
			return this.#t === -1 || !this.#a || !this.#E ? !1 : !this.#r || this.hasCallback() || this.#w === !1;
		}
		createSubtask(e) {
			x("[AsyncTask#createSubtask()] args", e);
			let { componentIdx: t, childTask: n, callMetadata: r, fnName: i, isAsync: a, isManualAsync: o } = e;
			if (!M(this.#t)) throw Error(`invalid/missing async state for component idx [${t}]`);
			let s = new S({
				componentIdx: this.#t,
				target: `subtask (internal ID [${this.#e}])`
			}), c = new B({
				componentIdx: t,
				childTask: n,
				parentTask: this,
				callMetadata: r,
				isAsync: a,
				isManualAsync: o,
				fnName: i,
				waitable: s
			});
			return this.#I.push(c), c.setTarget(`subtask (internal ID [${c.id()}], waitable [${s.idx()}], component [${t}])`), c;
		}
		getLatestSubtask() {
			return this.#I.at(-1);
		}
		getSubtaskByWaitableRep(e) {
			if (e === void 0) throw TypeError("missing rep");
			return this.#I.find((t) => t.waitableRep() === e);
		}
		currentSubtask() {
			if (x("[AsyncTask#currentSubtask()]"), this.#I.length !== 0) return this.#I.at(-1);
		}
		removeSubtask(e) {
			if (this.#I.length === 0) throw Error("cannot end current subtask: no current subtask");
			return this.#I = this.#I.filter((t) => t !== e), e;
		}
	}
	class Z {
		static State = {
			NOT_STARTED: "not-started",
			RUNNING: "running",
			COMPLETED: "completed"
		};
		#e = null;
		#t;
		#n;
		#r;
		#i = null;
		storage = [0, 0];
		constructor(e) {
			if (e?.componentIdx === void 0) throw TypeError("missing component id during thread creation");
			if (this.#t = e.componentIdx, !e?.task) throw TypeError("missing task during thread creation");
			this.#n = e.task, e.startFn ? (this.#i = e.startFn, this.#r = Z.State.NOT_STARTED) : this.#r = Z.State.RUNNING;
		}
		id() {
			return this.#e;
		}
		setId(e) {
			if (this.#e !== null) throw Error("thread id can only be assigned once");
			this.#e = e;
		}
		componentIdx() {
			return this.#t;
		}
		task() {
			return this.#n;
		}
		taskID() {
			return this.#n.id();
		}
		state() {
			return this.#r;
		}
		isCancelled() {
			return this.#n.isCancelled();
		}
		isRejected() {
			return this.#n.isRejected();
		}
		notifyProgress() {
			return this.#n.notifyProgress();
		}
		entryFnName() {
			return this.#n.entryFnName();
		}
		getParentSubtask() {
			return this.#n.getParentSubtask();
		}
		async suspend(e) {
			let t = M(this.#t), n = this.#n;
			n.endFirstSlice(), n.settleCompletion();
			let { cancellable: r, readyFn: i, releaseLock: a = !1 } = e;
			if (n.deliverPendingCancel({ cancellable: r })) return !1;
			let o = a && n.needsExclusiveLock() && t.exclusivelyLockedBy(n.id());
			o && t.exclusiveRelease(n.id());
			try {
				let a = await t.suspendThread({
					...e,
					thread: this,
					readyFn: () => r && n.cancellationPending() || i()
				});
				return t.throwIfInterrupted(), a && n.deliverPendingCancel({ cancellable: r }) ? !1 : a;
			} finally {
				o && !t.exclusivelyLockedBy(n.id()) && await t.acquireExclusiveLock(n.id());
			}
		}
		start() {
			if (this.#r !== Z.State.NOT_STARTED) throw Error(`cannot start thread [${this.#e}] in state [${this.#r}]`);
			this.#r = Z.State.RUNNING;
			let e = this.#i;
			this.#i = null, Y({
				threadID: this.#e,
				taskID: this.#n.id(),
				componentIdx: this.#t,
				fn: () => (M(this.#t).throwIfInterrupted(), e())
			}).then(() => {
				this.#r = Z.State.COMPLETED, this.#n.removeThread(this);
			}).catch((e) => {
				e = W(this.#t, e), this.#r = Z.State.COMPLETED, x("[AsyncThread#start()] spawned thread errored", {
					threadID: this.#e,
					taskID: this.#n.id(),
					componentIdx: this.#t,
					err: e
				}), this.#n.setErrored(e), this.#n.reject(e), this.#n.removeThread(this);
			});
		}
	}
	function ne(e) {
		x("[createNewCurrentImplicitThread()] args", e);
		let { componentIdx: t, isAsync: n, isManualAsync: r, preserveFutureResult: i, entryFnName: a, parentSubtaskID: o, callbackFnName: s, getCallbackFn: c, getParamsFn: l, stringEncoding: u, errHandling: d, getCalleeParamsFn: f, resultPtr: p, callingWasmExport: m } = e;
		if (t == null) throw Error("missing/invalid component instance index while starting task");
		let h = P.get(t), g = c ? c() : null, _ = new X({
			componentIdx: t,
			isAsync: n,
			isManualAsync: r,
			preserveFutureResult: i,
			entryFnName: a,
			callbackFn: g,
			callbackFnName: s,
			stringEncoding: u,
			getCalleeParamsFn: f,
			resultPtr: p,
			errHandling: d,
			callingWasmExport: m
		}), v = _.id(), y = {
			id: v,
			componentIdx: t,
			task: _
		}, b = _.addThread(new Z({
			componentIdx: t,
			task: _
		})), S = b.id();
		return F.push(v), ee.push(t), h ? h.push(y) : (h = [y], P.set(t, [y])), {
			task: _,
			thread: b,
			taskID: v,
			threadID: S
		};
	}
	let re = a !== void 0 && new a.Global({
		value: "i32",
		mutable: !0
	}, 0), ie = (e) => fetch(e).then(a.compileStreaming);
	function ae() {
		throw TypeError("invalid variant discriminant for bool");
	}
	if (!e) {
		let t = {
			"component.core.wasm": new URL("data:application/wasm;base64,AGFzbQEAAAABGQVgAX8Bf2AAAGABfwBgAAF/YAR/f39/AX8DCQgDAAABAgEEAgUFAQEBgAIGHwZ/AUEIC38BQQALfwFBAAt/AUEAC38BQQALfwFBAAsHYwkGbWVtb3J5AgALc2NvcGUtb3duZXIDBBBjYW5jZWwtcmVxdWVzdGVkAwUEbGl2ZQAABWFsbG9jAAIEZnJlZQAECGZyZWUtYWxsAAUHcmVhbGxvYwAGCmZyZWUtc2NvcGUABwquBQgEACMDCzMAIAAjBDYCECAAQQA2AgggACMCNgIMIwIEQCMCIAA2AggLIAAkAiMDQQFqJAMgAEEYagvrAQEEfyAAQdD//wdLBEAACyAAQR9qQXhxIQAjASEBAkADQCABRQ0BIAEoAgQgAE8EQCABKAIEIABrQSBPBEAgASAAaiEDIAMgASgCADYCACADIAEoAgQgAGs2AgQgASAANgIEIAIEQCACIAM2AgAFIAMkAQsFIAIEQCACIAEoAgA2AgAFIAEoAgAkAQsLIAEQAQ8LIAEhAiABKAIAIQEMAAsACyMAIQEgASAAaiEDIANBgICACEsEQAALIANB//8DakEQdiEEIAQ/AEsEQCAEPwBrQABBf0YEQAALCyADJAAgASAANgIEIAEQAQt7AQN/IwEhAAJAA0AgAEUNASAAKAIAIQIgAgRAIAAgACgCBGogAkYEQCAAIAAoAgQgAigCBGo2AgQgACACKAIANgIADAILIAAhASACIQAMAQsLIAAEQCAAIAAoAgRqIwBGBEAgACQAIAEEQCABQQA2AgAFQQAkAQsLCwsLgAEBBH8gAARAIABBGGshACAAKAIIIQEgACgCDCECIAEEQCABIAI2AgwFIAIkAgsgAgRAIAIgATYCCAsjA0EBayQDIwEhBAJAA0AgBEUNASAEIABLDQEgBCEDIAQoAgAhBAwACwALIAAgBDYCACADBEAgAyAANgIABSAAJAELEAMLCxMAA0AjAgRAIwJBGGoQBAwBCwsLRAEBfyACRSACQQhLcgRAAAsgA0UEQCAAEARBAA8LIAMQAiEEIAAEQCAEIAAgASADSQR/IAEFIAML/AoAACAAEAQLIAQLLwECfyMCIQEDQCABBEAgASgCDCECIAEoAhAgAEYEQCABQRhqEAQLIAIhAQwBCwsL", "" + import.meta.url),
			"component.core2.wasm": new URL("data:application/wasm;base64,AGFzbQEAAAABggZUThheeAFfBn8AfwB/AWMBAWMBAX8BXwNkCwB/AH8AXwVkCwB/AH8AfwB/AF8EZAsAfwB/AH8AXn8BXngBXn8BXwdkBgF/AX8BZAcBfwF/AX8BXwJ/AGQRAF8HfgBkEQBkEABkCwB/AHwAZBIAXwFkAABQAF8BfwBPAQxfAn8AZAoATwEMXwJ/AH8AXn8BXwFkDwBfAWQAAFAAXwF/AE8BEl8BfwBPARJfAn8AZAkAUABfAX8ATwEVXwF/AE8BFV8CfwBkEQBgAX8AYAAAYAJkCH8AYAF/AX9gAWQKAWQLYAFkCwFkDGAAAX9gA2QDf38Bf2ABZAgBf2ADZAh/fwBgAn9/AGACZAt/AGACZBB/AGACZBF/AGADZAt/fwFkC2ABZBEBZAtgAWQAAX9gAWQFAWQLYAFkBwFkC2ADZAZ/fwFkC2AFZAZ/ZAt/fwBgAWQLAWQVYAJkEH8Bf2ADZBB/fwFkEGABZAUBZBBgAWQLAWQCYAFkAgF/YANkAn9/AWQCYAFkAgFkC2AEZAt+fn8AYANkC35/AX5gAmQLfgFkA2ACZAtkCwFkA2AEZAN/f34BfmADZAN/fgF+YANkA398AXxgAmQDfwF/YAJkA38BZBFgA2QDf38BZARgAWQEAX9gAWQEAWQCYAJkC38Bf2ACZAN/AWQQYAABZAhgAWQIAGADZAZ/fwBgA2QIfn8AYANkCGQLfwF/YAJkCGQLAX9gAmQIZBEBf2ACZAhkEAF/YAVkCH9/fn4AYANkCH9kCwFkC2ACZAhkCQF/YAJkC38BZBJgAn9/AWQBYAFkAQBgAX8BfmACfn8AYAF/AX1gAn1/AGABfwF8YAJ8fwBgAX8BZAlgAmQJfwBgAX8BZApgAmQKfwBgAn9/AWQLYAFkCwF/YAF/AWQLYAF/AWQMYAJkDH8AYAJ/fwFkEGABfwFkEGACf38BZBFgAWQRAX9gAX8BZBFgAX8BZBJgAmQSfwBgAX8BZBVgAmQVfwBgDX5/f39/f39/fH9/f38Bf2ACf38BfwJ3BQtfX2NvbXBvbmVudAZtZW1vcnkCAQGAAgtfX2NvbXBvbmVudBBjYW5jZWwtcmVxdWVzdGVkA38BC19fY29tcG9uZW50BWFsbG9jABsLX19jb21wb25lbnQEZnJlZQAYC19fY29tcG9uZW50CGZyZWUtYWxs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", "" + import.meta.url)
		};
		e = (e) => ie(t[e]);
	}
	let oe = e("component.core.wasm");
	Promise.resolve(oe).catch(() => {});
	let se = e("component.core2.wasm");
	Promise.resolve(se).catch(() => {});
	let ce = (function* () {
		let e, t, r, i, o, u, p;
		function h(e) {
			M(0).throwIfTrapped();
			let { task: t, threadID: n } = ne({
				componentIdx: 0,
				isAsync: !1,
				isManualAsync: !1,
				preserveFutureResult: !1,
				entryFnName: "exports1Export0",
				getCallbackFn: () => null,
				callbackFnName: null,
				errHandling: "none",
				callingWasmExport: !0
			});
			return t.setCalleeIsAsync(!1), t.setFuncTypeIsAsync(!1), t.setReturnResultTypeIdx(1), t.enterSync(), re.value = +!!t.mayBlock(), t.setReturnMemoryIdx(0), t.setReturnMemory(r), J({
				taskID: t.id(),
				threadID: n,
				componentIdx: t.componentIdx(),
				fn: () => {
					try {
						var { id: a, name: u, values: f, payload: h, enabled: g, weight: _, child: v } = e, y = m(u, ((...e) => {
							let t = M(0), n = t.mayLeave, r = q(0), a = r && N(r.componentIdx, r.threadID, r.taskID)?.thread, o = a?.storage;
							a && (a.storage = [0, 0]), t.mayLeave = !1;
							let s = () => {
								t.mayLeave = n, a && (a.storage = o);
							}, c;
							try {
								c = i(...e);
							} catch (e) {
								throw s(), e;
							}
							return c && typeof c.then == "function" ? Promise.resolve(c).finally(s) : (s(), c);
						}), r), b = y.ptr, S = y.len, C = f, w = C.length, T = ((...e) => {
							let t = M(0), n = t.mayLeave, r = q(0), a = r && N(r.componentIdx, r.threadID, r.taskID)?.thread, o = a?.storage;
							a && (a.storage = [0, 0]), t.mayLeave = !1;
							let s = () => {
								t.mayLeave = n, a && (a.storage = o);
							}, c;
							try {
								c = i(...e);
							} catch (e) {
								throw s(), e;
							}
							return c && typeof c.then == "function" ? Promise.resolve(c).finally(s) : (s(), c);
						})(0, 0, 4, w * 4);
						let te, z = w * 4;
						if (Array.isArray(C)) {
							let e = 0, t = new DataView(r.buffer);
							for (let n of C) d.bind(null, "s32")(n), t.setInt32(T + e, n, !0), e += 4;
						} else te = new Uint8Array(C.buffer || C, C.byteOffset, z), new Uint8Array(r.buffer, T, z).set(te);
						var D = h, O = Array.isArray(D) ? D.length : D.byteLength, k = ((...e) => {
							let t = M(0), n = t.mayLeave, r = q(0), a = r && N(r.componentIdx, r.threadID, r.taskID)?.thread, o = a?.storage;
							a && (a.storage = [0, 0]), t.mayLeave = !1;
							let s = () => {
								t.mayLeave = n, a && (a.storage = o);
							}, c;
							try {
								c = i(...e);
							} catch (e) {
								throw s(), e;
							}
							return c && typeof c.then == "function" ? Promise.resolve(c).finally(s) : (s(), c);
						})(0, 0, 1, O * 1);
						let B, V = O * 1;
						if (Array.isArray(D)) {
							let e = 0, t = new DataView(r.buffer);
							for (let n of D) d.bind(null, "u8")(n), t.setUint8(k + e, n, !0), e += 1;
						} else B = new Uint8Array(D.buffer || D, D.byteOffset, V), new Uint8Array(r.buffer, k, V).set(B);
						var A = v;
						let U, W, G, K;
						if (A == null) U = 0, W = 0, G = 0, K = 0;
						else {
							var { count: j, label: P } = A, y = m(P, ((...e) => {
								let t = M(0), n = t.mayLeave, r = q(0), a = r && N(r.componentIdx, r.threadID, r.taskID)?.thread, o = a?.storage;
								a && (a.storage = [0, 0]), t.mayLeave = !1;
								let s = () => {
									t.mayLeave = n, a && (a.storage = o);
								}, c;
								try {
									c = i(...e);
								} catch (e) {
									throw s(), e;
								}
								return c && typeof c.then == "function" ? Promise.resolve(c).finally(s) : (s(), c);
							}), r), F = y.ptr, ee = y.len;
							U = 1, W = l(j), G = F, K = ee;
						}
						x("[iface=\"walz:generated/api@0.1.0\", function=\"encode\"][Instruction::CallWasm] enter", {
							funcName: "encode",
							paramCount: 13,
							async: !1,
							postReturn: !0
						});
						let Y;
						try {
							Y = J({
								taskID: t.id(),
								threadID: n,
								componentIdx: t.componentIdx(),
								fn: () => p(c(a), b, S, T, w, k, O, +!!g, +_, U, W, G, K)
							});
						} catch (e) {
							throw x("[Instruction::CallWasm] error during sync call", {
								taskID: t.id(),
								err: e
							}), M(0).markTrapped(e), t.setErrored(e), t.reject(e), t.exit(), e;
						}
						t.endFirstSlice();
						var I = s(r).getUint32((Y >>> 0) + 0, !0) >>> 0, L = s(r).getUint32((Y >>> 0) + 4, !0);
						if (I < 0 || L < 0 || L > 268435455 || I > r.buffer.byteLength || L > Math.floor((r.buffer.byteLength - I) / 1)) throw new E("wasm trap: out of bounds memory access");
						if (I % 1 != 0) throw TypeError(`list pointer [${I}] is not aligned to 1`);
						var R = new Uint8Array(r.buffer.slice(I, I + L * 1));
						x("[iface=\"walz:generated/api@0.1.0\", function=\"encode\"][Instruction::Return]", {
							funcName: "encode",
							paramCount: 1,
							async: !1,
							postReturn: !0
						}), t.resolve([R]);
						let X = R, Z = M(0);
						Z.mayLeave = !1;
						try {
							o(Y);
						} catch (e) {
							throw e = H(0, e), t.reject(e), e;
						}
						return Z.mayLeave = !0, t.exit(), X;
					} catch (e) {
						throw t.isResolvedState() || (t.setErrored(e), t.reject(e)), t.isExited() || t.exit({ skipExclusiveLockCheck: !0 }), e;
					}
				}
			});
		}
		let g;
		function _(e) {
			M(0).throwIfTrapped();
			let { task: t, threadID: n } = ne({
				componentIdx: 0,
				isAsync: !1,
				isManualAsync: !1,
				preserveFutureResult: !1,
				entryFnName: "exports1Export1",
				getCallbackFn: () => null,
				callbackFnName: null,
				errHandling: "none",
				callingWasmExport: !0
			});
			return t.setCalleeIsAsync(!1), t.setFuncTypeIsAsync(!1), t.setReturnResultTypeIdx(2), t.enterSync(), re.value = +!!t.mayBlock(), t.setReturnMemoryIdx(0), t.setReturnMemory(r), J({
				taskID: t.id(),
				threadID: n,
				componentIdx: t.componentIdx(),
				fn: () => {
					try {
						var a = e, o = Array.isArray(a) ? a.length : a.byteLength, c = ((...e) => {
							let t = M(0), n = t.mayLeave, r = q(0), a = r && N(r.componentIdx, r.threadID, r.taskID)?.thread, o = a?.storage;
							a && (a.storage = [0, 0]), t.mayLeave = !1;
							let s = () => {
								t.mayLeave = n, a && (a.storage = o);
							}, c;
							try {
								c = i(...e);
							} catch (e) {
								throw s(), e;
							}
							return c && typeof c.then == "function" ? Promise.resolve(c).finally(s) : (s(), c);
						})(0, 0, 1, o * 1);
						let O, k = o * 1;
						if (Array.isArray(a)) {
							let e = 0, t = new DataView(r.buffer);
							for (let n of a) d.bind(null, "u8")(n), t.setUint8(c + e, n, !0), e += 1;
						} else O = new Uint8Array(a.buffer || a, a.byteOffset, k), new Uint8Array(r.buffer, c, k).set(O);
						x("[iface=\"walz:generated/api@0.1.0\", function=\"decode\"][Instruction::CallWasm] enter", {
							funcName: "decode",
							paramCount: 2,
							async: !1,
							postReturn: !0
						});
						let A;
						try {
							A = J({
								taskID: t.id(),
								threadID: n,
								componentIdx: t.componentIdx(),
								fn: () => g(c, o)
							});
						} catch (e) {
							throw x("[Instruction::CallWasm] error during sync call", {
								taskID: t.id(),
								err: e
							}), M(0).markTrapped(e), t.setErrored(e), t.reject(e), t.exit(), e;
						}
						t.endFirstSlice();
						let j;
						switch (s(r).getUint8((A >>> 0) + 0, !0)) {
							case 0: {
								var l = s(r).getUint32((A >>> 0) + 16, !0) >>> 0, p = s(r).getUint32((A >>> 0) + 20, !0), m = f.decode(new Uint8Array(r.buffer, l, p)), h = s(r).getUint32((A >>> 0) + 24, !0) >>> 0, _ = s(r).getUint32((A >>> 0) + 28, !0);
								if (h < 0 || _ < 0 || _ > 67108863 || h > r.buffer.byteLength || _ > Math.floor((r.buffer.byteLength - h) / 4)) throw new E("wasm trap: out of bounds memory access");
								if (h % 4 != 0) throw TypeError(`list pointer [${h}] is not aligned to 4`);
								var v = new Int32Array(r.buffer.slice(h, h + _ * 4)), y = s(r).getUint32((A >>> 0) + 32, !0) >>> 0, b = s(r).getUint32((A >>> 0) + 36, !0);
								if (y < 0 || b < 0 || b > 268435455 || y > r.buffer.byteLength || b > Math.floor((r.buffer.byteLength - y) / 1)) throw new E("wasm trap: out of bounds memory access");
								if (y % 1 != 0) throw TypeError(`list pointer [${y}] is not aligned to 1`);
								var S = new Uint8Array(r.buffer.slice(y, y + b * 1)), C = s(r).getUint8((A >>> 0) + 40, !0);
								let e;
								switch (s(r).getUint8((A >>> 0) + 56, !0)) {
									case 0:
										e = void 0;
										break;
									case 1:
										var w = s(r).getUint32((A >>> 0) + 64, !0) >>> 0, T = s(r).getUint32((A >>> 0) + 68, !0), D = f.decode(new Uint8Array(r.buffer, w, T));
										e = {
											count: s(r).getInt32((A >>> 0) + 60, !0),
											label: D
										};
										break;
									default: throw TypeError("invalid variant discriminant for option");
								}
								j = {
									tag: "ok",
									val: {
										id: s(r).getBigInt64((A >>> 0) + 8, !0),
										name: m,
										values: v,
										payload: S,
										enabled: C == 0 ? !1 : C == 1 || ae(),
										weight: s(r).getFloat64((A >>> 0) + 48, !0),
										child: e
									}
								};
								break;
							}
							case 1:
								j = {
									tag: "err",
									val: s(r).getInt32((A >>> 0) + 8, !0)
								};
								break;
							default: throw TypeError("invalid variant discriminant for expected");
						}
						x("[iface=\"walz:generated/api@0.1.0\", function=\"decode\"][Instruction::Return]", {
							funcName: "decode",
							paramCount: 1,
							async: !1,
							postReturn: !0
						}), t.resolve([j]);
						let P = j, F = M(0);
						F.mayLeave = !1;
						try {
							u(A);
						} catch (e) {
							throw e = H(0, e), t.reject(e), e;
						}
						return F.mayLeave = !0, t.exit(), P;
					} catch (e) {
						throw t.isResolvedState() || (t.setErrored(e), t.reject(e)), t.isExited() || t.exit({ skipExclusiveLockCheck: !0 }), e;
					}
				}
			});
		}
		let b;
		function S() {
			M(0).throwIfTrapped();
			let { task: e, threadID: t } = ne({
				componentIdx: 0,
				isAsync: !1,
				isManualAsync: !1,
				preserveFutureResult: !1,
				entryFnName: "exports1Export2",
				getCallbackFn: () => null,
				callbackFnName: null,
				errHandling: "none",
				callingWasmExport: !0
			});
			return e.setCalleeIsAsync(!1), e.setFuncTypeIsAsync(!1), e.setReturnResultTypeIdx(4), e.enterSync(), re.value = +!!e.mayBlock(), e.setReturnMemoryIdx(0), e.setReturnMemory(r), J({
				taskID: e.id(),
				threadID: t,
				componentIdx: e.componentIdx(),
				fn: () => {
					try {
						x("[iface=\"walz:generated/api@0.1.0\", function=\"main\"][Instruction::CallWasm] enter", {
							funcName: "main",
							paramCount: 0,
							async: !1,
							postReturn: !1
						});
						let n;
						try {
							n = J({
								taskID: e.id(),
								threadID: t,
								componentIdx: e.componentIdx(),
								fn: () => b()
							});
						} catch (t) {
							throw x("[Instruction::CallWasm] error during sync call", {
								taskID: e.id(),
								err: t
							}), M(0).markTrapped(t), e.setErrored(t), e.reject(t), e.exit(), t;
						}
						return e.endFirstSlice(), x("[iface=\"walz:generated/api@0.1.0\", function=\"main\"][Instruction::Return]", {
							funcName: "main",
							paramCount: 1,
							async: !1,
							postReturn: !1
						}), e.resolve([n]), e.exit(), n;
					} catch (t) {
						throw e.isResolvedState() || (e.setErrored(t), e.reject(t)), e.isExited() || e.exit({ skipExclusiveLockCheck: !0 }), t;
					}
				}
			});
		}
		({exports: e} = yield n(yield oe)), {exports: t} = yield n(yield se, { __component: {
			alloc: Object.assign(e.alloc, { _jcoMaySuspend: !1 }),
			"cancel-requested": e["cancel-requested"],
			free: Object.assign(e.free, { _jcoMaySuspend: !1 }),
			"free-all": Object.assign(e["free-all"], { _jcoMaySuspend: !1 }),
			memory: e.memory
		} }), r = e.memory, i = (t, n, r, i) => e.realloc(t, n, r, i) >>> 0;
		try {
			a.promising(e.realloc);
		} catch {}
		o = t.__post_0;
		try {
			a.promising(t.__post_0);
		} catch {
			t.__post_0;
		}
		u = t.__post_1;
		try {
			a.promising(t.__post_1);
		} catch {
			t.__post_1;
		}
		p = t.__export_0, g = t.__export_1, b = t.__export_2;
		let C = {
			decode: _,
			encode: h,
			main: S
		};
		return {
			__memory: () => ({
				scopes: v === void 0 ? 0 : v.size,
				bufferRegistry: typeof BUFFER_MGR > "u" ? 0 : BUFFER_MGR.__walzSize(),
				tasks: P === void 0 ? 0 : Array.from(P.values()).reduce((e, t) => e + t.length, 0),
				futures: typeof FUTURES > "u" ? 0 : FUTURES.size(),
				streams: typeof STREAMS > "u" ? 0 : STREAMS.size(),
				futureEnds: typeof FUTURE_TABLES > "u" ? 0 : Object.values(FUTURE_TABLES).reduce((e, t) => e + t.table.size(), 0),
				streamEnds: typeof STREAM_TABLES > "u" ? 0 : Object.values(STREAM_TABLES).reduce((e, t) => e + t.table.size(), 0),
				handles: y === void 0 ? 0 : Array.from(y.values()).reduce((e, t) => e + t.handles.size(), 0),
				threads: y === void 0 ? 0 : Array.from(y.values()).reduce((e, t) => e + t.threads.size(), 0),
				subtasks: y === void 0 ? 0 : Array.from(y.values()).reduce((e, t) => e + t.subtasks.size(), 0)
			}),
			api: C,
			"walz:generated/api@0.1.0": C
		};
	})(), le, Q, $;
	function ue(e) {
		return a !== void 0 && typeof a.SuspendError == "function" && e instanceof a.SuspendError ? new a.RuntimeError("cannot block a synchronous task before returning") : e;
	}
	function de(e) {
		try {
			let t;
			do
				({value: e, done: t} = ce.next(e));
			while (!(e instanceof Promise) && !t);
			if (t) return Q ? Q(e) : e;
			le ||= new Promise((e, t) => (Q = e, $ = t)), e.then((e) => t ? Q() : de(e), (e) => $(ue(e)));
		} catch (e) {
			if (e = ue(e), $) $(e);
			else throw e;
		}
	}
	let fe = de(null);
	return le || fe;
}
// benchmarks/flatbuffers/20261111-three-improvements-v1/sizes/component-strip/host.mjs
function a({ onEvent: e = () => {} } = {}) {
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
function o(e) {
	let t = {};
	for (let { id: n, kind: r, unit: i } of e) t[`readyT${n}`] = (e) => {
		let t = Array.from(e, (e) => i ? void 0 : e);
		if (r === "future") {
			if (t.length !== 1) throw TypeError("future requires one value");
			return Promise.resolve(t[0]);
		}
		return s(t);
	}, r === "stream" && (t[`prependT${n}`] = (e, t) => s(Array.from(e, (e) => i ? void 0 : e), t));
	return t;
}
function s(e, t) {
	let n = 0, r = !1, i = t?.[Symbol.asyncIterator]();
	return {
		[Symbol.asyncIterator]() {
			return this;
		},
		async next() {
			if (r) return { done: !0 };
			if (n < e.length) {
				let t = e[n];
				return e[n++] = void 0, n === e.length && (e.length = 0, n = 0), {
					done: !1,
					value: t
				};
			}
			let t = i ? await i.next() : { done: !0 };
			return t.done && (r = !0, i = null), t;
		},
		async readMany(t) {
			if (!Number.isInteger(t) || t < 1 || t > 65536) throw RangeError("invalid batch count");
			if (r) return {
				items: [],
				done: !0
			};
			if (n < e.length) {
				let r = Math.min(e.length, n + t), i = e.slice(n, r);
				return e.fill(void 0, n, r), n = r, {
					items: i,
					done: !1
				};
			}
			let a = i?.readMany ? await i.readMany(t) : i ? await Promise.resolve(i.next()).then((e) => ({
				items: e.done ? [] : [e.value],
				done: e.done
			})) : {
				items: [],
				done: !0
			};
			return a.done && (r = !0, i = null), a;
		},
		async return() {
			if (!r) {
				r = !0, e.length = 0, n = 0;
				let t = i;
				i = null, await t?.return?.();
			}
			return { done: !0 };
		}
	};
}
function c(e, t, n = []) {
	let r = "walz:host/event-loop@0.1.0", i = Object.assign(Object.create(null), e.imports);
	i[r] = t.host(i[r] ?? e.eventLoop ?? a());
	let o = { ...i };
	for (let e of n.filter((e) => e.resource)) {
		let { kind: r, operation: a } = e.resource, s = o[e.module], c = s?.[e.jsName];
		if (typeof c != "function") throw TypeError(`missing resource provider ${e.module}#${e.name}`);
		let l = (...e) => c.apply(s, e);
		if (i[e.module] = { ...i[e.module] }, a === "new") {
			let a = n.find((e) => e.resource?.kind === r && e.resource.operation === "close"), s = o[a?.module], c = s?.[a?.jsName];
			if (typeof c != "function") throw TypeError(`missing destructor for ${r}`);
			i[e.module][e.jsName] = (...e) => t.resources.add(r, l(...e), (e) => c.call(s, e));
		} else if (a === "use" || a === "reader") i[e.module][e.jsName] = (e, ...n) => t.resources.use(r, e, l, n);
		else if (a === "close") i[e.module][e.jsName] = (e) => t.resources.close(r, e);
		else throw TypeError(`unsupported resource operation ${a}`);
	}
	for (let e of new Set(n.map((e) => e.module))) e !== r && i[e] && (i[e] = t.host(i[e], n.filter((t) => t.module === e && t.effects.suspends).map((e) => e.jsName)));
	for (let e of n) if (typeof i[e.module]?.[e.jsName] != "function") throw TypeError(`missing host capability ${e.module}#${e.name}`);
	for (let [e, t] of Object.entries(i)) i[e.replace(/@[^@]+$/, "")] = t;
	return i;
}
// benchmarks/flatbuffers/20261111-three-improvements-v1/sizes/component-strip/resources.mjs
var l = class {
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
// benchmarks/flatbuffers/20261111-three-improvements-v1/sizes/component-strip/tasks.mjs
function u(e, t) {
	if (!(e instanceof Error) || (t ??= /* @__PURE__ */ new Set(), t.has(e))) return;
	t.add(e);
	try {
		e.stack;
	} catch {}
	u(Object.getOwnPropertyDescriptor(e, "cause")?.value, t);
	let n = Object.getOwnPropertyDescriptor(e, "errors")?.value;
	if (Array.isArray(n)) for (let e of n) u(e, t);
}
var d = class extends Error {
	constructor(e, t = "Task cancelled", n = "TaskCancelledError") {
		super(t, { cause: e }), this.name = n, u(this);
	}
}, f = class extends d {
	constructor(e) {
		super(void 0, `Task timed out after ${e} ms`, "TaskTimeoutError"), this.timeoutMs = e;
	}
}, p = (e) => e instanceof f ? e : new d(e), m = /* @__PURE__ */ new Set([
	"returned",
	"cancelled",
	"failed"
]), h = Object.getPrototypeOf(async function() {}), g = () => {};
function _() {
	return Promise.withResolvers();
}
function v(e) {
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
var y = class {
	constructor({ onTaskEvent: e = () => {}, asyncExports: t = [] } = {}) {
		this.notify = e, this.asyncExports = new Set(t), this.queue = [], this.active = null, this.failure = null, this.nextId = 1, this.resources = new l(() => this.active, (e) => this.disposal(e), () => this.scope), this.children = null, this.scope = 0;
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
		for (let [e, t] of Object.entries(this.raw)) typeof t == "function" && Object.getPrototypeOf(t) === h && this.asyncExports.add(e);
		let r = Object.fromEntries(Object.entries(this.raw).map(([e, t]) => [e, typeof t == "function" ? (...t) => this.invoke(e, t) : t]));
		return {
			...n,
			api: r,
			"walz:generated/api@0.1.0": r,
			start: this.start.bind(this)
		};
	}
	create(e, t, n = {}, r = !1) {
		if (!Object.hasOwn(this.raw, e) || typeof this.raw[e] != "function") throw TypeError(`unknown export ${e}`);
		if (!Array.isArray(t)) throw TypeError("task arguments must be an array");
		if (n.timeoutMs !== void 0 && (!Number.isInteger(n.timeoutMs) || n.timeoutMs < 0 || n.timeoutMs > 2147483647)) throw RangeError("invalid timeoutMs");
		if (n.signal && typeof n.signal.addEventListener != "function") throw TypeError("expected AbortSignal");
		let i = r && !this.asyncExports.has(e) ? null : _(), a = i ? _() : null;
		i?.promise.then(g, g);
		let o = {
			id: this.nextId++,
			name: e,
			args: r ? t : [...t],
			state: "queued",
			started: !1,
			settling: !1,
			reason: void 0,
			result: i,
			done: a,
			disposals: [],
			reads: /* @__PURE__ */ new Set(),
			blocking: /* @__PURE__ */ new Set()
		};
		if (!i) return o;
		let s = (e) => {
			if (m.has(o.state) || o.settling || o.state === "cancelling") return a.promise;
			if (o.reason = e, !o.started) return o.state = "cancelled", o.cancel = void 0, o.detach?.(), o.detach = void 0, o.args = [], i.reject(p(e)), a.resolve({
				status: "cancelled",
				started: !1
			}), a.promise;
			o.state = "cancelling", this.flag.value = 1;
			for (let e of o.reads) e();
			for (let e of o.blocking) e();
			return a.promise;
		};
		o.cancel = s, o.public = v(o);
		let c, l = n.signal, u = () => s(l.reason);
		return o.detach = () => {
			clearTimeout(c), l?.removeEventListener("abort", u);
		}, l?.addEventListener("abort", u, { once: !0 }), l?.aborted && u(), !m.has(o.state) && n.timeoutMs !== void 0 && (n.timeoutMs === 0 ? s(new f(0)) : c = setTimeout(() => s(new f(n.timeoutMs)), n.timeoutMs)), o;
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
		let n = this.create(e, t, {}, !0);
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
	publish(e, t, n, r = !1) {
		u(n), e.state = r ? "cancelled" : n ? "failed" : "returned", e.cancel = void 0, e.detach?.(), e.detach = void 0, e.result && (r ? e.result.reject(p(e.reason)) : n ? e.result.reject(n) : typeof t?.resolveAsValue == "function" ? t.resolveAsValue(e.result.resolve) : e.result.resolve(t)), e.done?.resolve({
			status: e.state,
			started: e.started
		}), this.active = null, this.flag.value = 0, this.queue.length && queueMicrotask(() => this.pump());
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
}, b = [], x = [].map((e) => e.replace(/-([a-z0-9])/g, (e, t) => t.toUpperCase()));
async function S(t = {}) {
	if (typeof WebAssembly.Suspending != "function" || typeof WebAssembly.promising != "function") throw Error("WebAssembly JSPI is required");
	let n = new y({
		...t,
		asyncExports: x
	}), r = o(b), a = {
		...c(t, n, []),
		"walz:runtime/delivery": r,
		"walz:runtime/delivery@0.1.0": r,
		"walz:runtime/tasks": n.children?.host() ?? {},
		"walz:runtime/tasks@0.1.0": n.children?.host() ?? {}
	}, s = t.getCoreModule;
	return !s && typeof process < "u" && process.versions?.node && (s = async (t) => WebAssembly.compile(await (await import("./__vite-browser-external-q4xEsQeL.js").then((t) => /* @__PURE__ */ e(t.default, 1))).readFile(new URL(t, import.meta.url)))), n.bind(await i(s, a, async (e, r) => n.capture(await (t.instantiateCore ?? WebAssembly.instantiate)(e, r)), { walz: {
		localCancellation: (e, t) => n.active?.state !== "cancelling" && !!n.active?.children?.scopes.get(t)?.cancelled && e instanceof WebAssembly.Exception && e.is(n.tag),
		scope: () => n.scope,
		enter: (e) => n.enter(e),
		read: n.read.bind(n),
		disposal: n.disposal.bind(n),
		cleaning: () => n.cleaning === !0,
		childrenContexts: (e) => {
			n.clearChildContexts = e;
		}
	} }));
}
export { S as instantiate };

//# sourceMappingURL=walz-runtime.js.map