# Native WIT worlds

`walz build main.walz --target component --wit-input api.wit --world guest -o dist/app.component.wasm` builds a Component with the original WIT interface names and type identities. With generated imports, also pass `--bindings generated/bindings.json`. `walz wit-bindgen api.wit policy.json generated --world guest` generates typed imports, types, and export stubs; implement the stubs before building. Interface exports map to source exports named `interface-function`.

This boundary uses the standard Canonical ABI. The resulting Component does not import Walz's private delivery runtime. The executable inputs are [async](../tests/component-fixtures/native-async.walz), [async WIT](../tests/component-fixtures/native-async.wit), [resources](../tests/component-fixtures/native-resources.walz), and [resource WIT](../tests/component-fixtures/native-resources.wit).

For source-defined composition, use `(component ...)`, `@expose`, `@connect`, and `@world`. `build --wit` emits the checked public WIT; `--wit-input` consumes an existing contract. See [Component composition](component-composition.md).

## Async values

Async WIT functions and shared-value `future<T>` / `stream<T>` payloads use the existing Walz types and operations. An internal producer Component uses canonical future/stream creation, write, drop, and task return. It copies input values into its memory, publishes the readable end, and retains the write buffer until the receiver acknowledges the write. A prepended stream forwards the tail with partial-write handling. GC references are copied as values at the boundary; their identity is not portable.

The pinned jco transpiler in JSPI mode runs the generated Component without the Walz SDK or its scheduler/lifetime patches. The native build corrects duplicated async host-result allocations and propagates rejected host calls through JSPI. It retires only the unused serialized copy, using the original guarded allocator; it does not free pending copies. Numeric streams are delivered as typed-array chunks by that transpiler. The tests consume those chunks rather than treating them as scalar items. Walz's private child scheduler, SDK cancellation/deadline API, and writer-host capabilities are not native world operations; a world requiring those imports is rejected.

## Resource ownership

Imported WIT resources keep their nominal Canonical ABI identity, including aliases reexported by another interface. Generated source types are `@resource $R` for an owned handle and `@borrow $R` for a borrowed parameter. An owner is consumed with `@move`, `@resource.close`, or a transfer through a typed boundary. A borrow cannot escape as an owner or be closed. This does not introduce general reference lifetimes.

Native factories and functions must use typed own/borrow signatures. Numeric factory bindings are rejected: regenerate them with `walz wit-bindgen`. The generator derives a `[resource-drop]name` operation from the granted resource authority; the adapter connects it to `canon resource.drop` rather than requiring a host function of that name. Host destructors run when ownership is explicitly closed. Transferring an owner removes the guest's cleanup registration, so it cannot close the same handle twice.

Owned resources may appear in the supported `option`, `result`, and two-element pair forms. Lists, ABI records/variants, and future/stream payloads remain restricted to shared values. Ordinary Core＋JSPI and SDK Component value adapters reject typed resource parameters/results; their existing SDK resource capabilities are a separate interface.

## Execution and checks

Native execution was checked with Wasmtime 50.0.1 using these experimental features:

```text
gc=y,function-references=y,exceptions=y,component-model-async=y,component-model-more-async-builtins=y,component-model-async-stackful=y
```

The dedicated engine test is ignored by the default Rust suite because older installed engines can accept the syntax while implementing an incompatible experimental ABI. Run it explicitly with a matching engine:

```sh
WALZ_WASMTIME=/path/to/wasmtime just test-native-engine
just test-native-worlds
```

The engine test reads a native future through a separate consumer Component. Node and browser tests also check pending input, Unicode payloads, stream forwarding, resource identity, own/borrow transfer, one destructor call, and allocation/table retirement. These are regression checks, not a proof of the runtime implementation.

Native trap and cancellation behavior follows the VM's Canonical ABI. A trap can poison an instance; the host must dispose of the instance/store and its resources. The Walz SDK's cleanup and timeout guarantees do not extend to arbitrary native hosts. Full WASI resource worlds, networking bindings, owned payload delivery, and error-context support remain future work. See [the Component Model concurrency design](https://github.com/WebAssembly/component-model/blob/main/design/mvp/Concurrency.md) and [the tested Wasmtime release](https://github.com/bytecodealliance/wasmtime/releases/tag/v50.0.1).
