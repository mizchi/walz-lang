# Typed Macros and WAT Instructions

Standard WAT modules continue to use the Wasm validator. Within typed Walz functions, numeric/SIMD linear-memory instructions can also be mixed with `@let`, control flow, and GC.

```wat
(module
  (memory $data 1)
  (func $main (export "main") (result i32) (@effect reads writes)
    (@let $pointer (type i32) 16
      (block (result i32)
        (i32.store $data $pointer (i32.const 42))
        (i32.load $data $pointer)))))
```

Supported operations include scalar/SIMD load/store, splat and lane load/store, `memory.copy`/`memory.fill`. Indices are memory32 i32 values. File modules resolve explicit memory names, and DCE preserves those dependencies. This memory is separate from JS ABI memory; emitted instructions always include memory names too.

Input/result types are checked against the operand stack of `wasmparser::FuncValidator` using small validation modules. Alignment, lanes, offsets, and address/value types also follow Wasm rules. Instruction signatures are not independently redefined.

Effect classification uses an explicit closed instruction list: loads have reads, store/fill has writes, and copy has both; each can physically trap. Analysis treats these as reads/writes of unknown linear memory, without assuming purity or non-aliasing. `@race-free` rejects raw accesses.

Instruction operands are checked numeric values. Owned values, borrows, and GC refs cannot pass into opaque instructions; newtypes require explicit unwrap. Existing ownership/effect checks apply inside checked expressions returning numbers.

Arbitrary stack regions, general opaque escapes for table/call/ref instructions, multiple results, memory64, and passive data are outside this bridge. Unknown instructions in typed extensions are rejected. This does not narrow acceptance of modules containing only ordinary WAT.
