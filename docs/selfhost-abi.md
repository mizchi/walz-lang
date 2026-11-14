# Checked bootstrap ABI views

The Walz bootstrap frontend checks a source-level subset of `@abi.type` before
specialization. It retains immutable shape/layout facts and removes only the
certified annotations from Core syntax. The independent instruction validator
still checks the complete expanded program before binary emission.

This is metadata validation for the Core backend. WIT generation, Component
output and host lift/lower adapters are separate remaining work.

## Supported views

```wat
(module
  (type $Point (struct (field $x i32) (field $y i64)))
  (@abi.type $Point (record "x" "y"))
  (func $entry (export "f") (result i32)
    (@let $p (struct.new $Point 20 (i64.const 22))
      (+ (struct.get $Point $x $p)
         (i32.wrap_i64 (struct.get $Point $y $p))))))
```

The view's shape is `(record (x i32) (y i64))`, with memory32 size 16, alignment
8 and two flat slots. The same program without the annotation emits identical
Core bytes. For a positional immutable struct, `(@abi.type $Pair (tuple))`
derives its fields in declaration order.

A target must be a closed, named, nonempty struct with immutable fields.
Records require named fields and string labels in the same order. Labels match
the field name after removing leading `$` characters; they must be unique
lowercase ASCII kebab identifiers, with digits allowed after a segment's first
letter. Tuple views take no labels. A target may have only one view.

Portable fields include:

- i32/i64 and f32/f64 shape facts. Float facts do not add float instructions to
  the independent Core backend; unsupported storage/instructions still fail.
- Trusted `walz:core` bytes, string, unit, list, option, result and tuple types,
  including imported aliases. Recognizing a spelling such as `$bytes` does not
  grant a user-defined type the core representation.
- Non-null references to another annotated nominal struct, closed value aliases
  and unconstrained generic value aliases whose substituted contents are
  portable. Type arguments are checked even when an alias does not use them.

The current core containers require nonunit payloads. Raw arrays, unannotated
structs, nullable references, recursive ABI graphs (also through a list), owned
or borrowed resources, Future/Stream, function/external references and v128 are
rejected. Newtype/enum/flags/variant views, generic declaration views, heap aliases
inside `ref` and annotations nested inside `rec` are outside this subset.
Core recursion remains available when no portable view is requested.

An opaque type's provider may define its view. An importing file cannot attach
a view to the foreign representation, including a view with incorrect labels.
The Rust compiler applies the same representation-access gate before validating
the view. Distinct module definitions retain distinct nominal identities even
when their names and shapes match.

## Contract and limits

[`abi.walz`](../walz/frontend/abi.walz) exports an opaque `$abi-checked` token and
immutable `$abi-view` facts: target, kind (0 record / 1 tuple), shape, size,
alignment and flat-slot count. `abi.contains` authorizes removal by exact tree
identity; a reconstructed or nested annotation does not inherit permission.
Only the validator can construct this token. It does not certify instruction
types or bypass the independent checker.

Facts use memory32 layout: fields align in order, a final record/tuple rounds
to its maximum alignment, bytes/string/list use pointer/length slots, and
Option/Result include a tag and joined payload slots. Shape strings use the
compiler's structural format (`text` and `pair` included), rather than emitted
WIT. Unit has the current compiler's logical one-byte/one-slot carrier.

Request-local memoization distinguishes active and completed nodes, rejects
cycles, and publishes only successful facts. No facts survive a request or
cross nominal identities. Repeated DAG edges reuse completed facts while their
expanded shapes still count against the limits.

| Bound | Maximum |
| --- | ---: |
| Source views | 1,024 |
| Fields per view / flat slots per value | 1,024 |
| Inline layout size | 16 MiB |
| Shape bytes per value | 64 KiB |
| Total memoized shape bytes | 8 MiB |
| Structural depth | Less than 64 |
| Resolution/substitution frames | 128 |
| Memo entries | 3,072 |
| Resolution/substitution visits | 1,048,576 |

The pass also inherits the source parser/linker's limits. These are finite
implementation bounds, not a proof of peak memory or elapsed time. The ABI
pass's resolution budget is separate from profiler v4's elaboration counters;
elapsed frontend time includes the pass.

Error 225 identifies malformed, unsupported, recursive or nonportable views;
226 identifies a bound violation. Foreign opaque access retains code 202.
Diagnostics refer to the original annotation's file and half-open UTF-8 range.
Ordinary instruction/type failures retain the independent checker's codes.

## Repeat the checks

```sh
just test-selfhost-abi
just test-selfhost-parity
just test-selfhost
just test-selfhost-e2e
cargo test --locked --test interface_types --test modules --test component
```

The v9 correctness cases cover record/tuple facts, core carriers, aliases,
distinct module identities, provider-owned opaque views, original source ranges,
invalid metadata, DAG/depth/shape limits and continued body checking. Both
adapters validate and execute emitted Wasm and check ABI buffer/handle retirement
after success and rejection. Those counters do not prove collection of every
guest GC object. No async adapters are added by this pass.
