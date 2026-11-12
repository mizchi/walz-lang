# Built-In `walz:core` and Multiple Bindings

Walz's basic types and standard functions use the same name-resolution, aliasing, and visibility rules as ordinary static imports.
The [generated standard API listing](stdlib-api.md) documents public names, complete function types, type bounds, and effects.
See [naming and WIT mapping](naming.md) for types, associated functions, and the distinction from WIT built-ins.
See [collections](collections.md) for generic text-key dictionaries and builders for immutable lists.
See [zero-copy views](views.md) for types sharing subranges of bytes/string and arbitrary GC arrays.
See [fixed-width byte access](byte-access.md) for endian-specific bytes/view readers and mutable packed-array writers.
See [checked integers](checked-integers.md) for size/offset addition, multiplication, and alignment with explicit Option failure.

```wat
(module
  (@use "walz:core"
    (type $string)
    (type $option)
    (type $result $Outcome)
    (type $future)
    (func $string.u64 $format))
  (func $main
    (export "main")
    (result (@inst $Outcome $string i32))
    (@effect reads writes allocates suspends)
    (@let
      (($pending (type (@inst $future i64)) (@future.ready i64 42))
       ($number (@future.await i64 (@move $pending)))
       ($label (call $format (local.get $number))))
      (@result.ok (local.get $label))))
)
```

`walz:core` is bundled with the compiler; it is neither a file path nor a Wasm host import. String-based check/compile APIs and file-based compilation resolve the same module. Resolution adds no network/file search or runtime module initialization.

| Imported type | Usage |
| --- | --- |
| `$string` / `$bytes` / `$unit` | No type arguments; write the name in type positions |
| `$option` / `$list` | `(@inst $option T)` / `(@inst $list T)` |
| `$result` / `$tuple` | `(@inst $result T E)` / `(@inst $tuple T U)` |
| `$box` / `$future` / `$stream` | `(@inst $box T)` / `(@inst $future T)` / `(@inst $stream T)` |
| `$task` | `(@inst $task T)`: an owned handle to a child task returning shareable result T |
| `$resource-handle` / `$borrow` | `(@inst $resource-handle $R)` / `(@inst $borrow $R)` for nominal resource type R |

These are transparent aliases for existing checked base types. Parameterized types use the same monomorphization as user-defined generic aliases, adding no nominal identity or runtime. `i32`, `i64`, `f32`, `f64`, and `ref` remain ordinary Wasm syntax.

Select standard functions to import with forms such as `(func $string.u64)`, `(func $list.map)`, and `(func $string-builder.new)`. Any local alias is allowed. Functions not imported do not appear in the caller's scope. Dependent standard modules expand once and undergo ordinary type, effect, ownership, and opaque checks and DCE. Internal representations of builders and JSON values are private.

Repeated imports of the same standard-name/alias pair in one scope share a binding. This also applies when `@include` inserts self-contained declarations. Assigning different standard names to the same local name or adding a same-named local declaration is rejected as a collision.

Fixed syntax such as `@string.lit`, `@result.ok`, `@option.some`, and `@future.ready`/`@future.await`/`@future.close` is compiler value operations. Type references use imported type names. In `(result (@inst $result T E))`, the outer result is a WAT result declaration; the inner form is one Result value type.

## Multiple-Binding `@let`

```text
(@let (($a [(type T)] initial-a)
       ($b [(type U)] initial-b)
       ...)
  body)
```

Initializers evaluate once each from left to right. `initial-b` can refer to `$a`; `initial-a` cannot refer to the later `$b`. When rebinding the same name, its initializer sees the previous binding, and the new binding becomes visible afterward. All names are in scope in the body and do not escape the expression.

Each binding may omit its `(type T)` annotation. `($n (type i64) 42)` initializes an i64 constant; `($copy $n)` uses the existing local's type. `($n 42)` without an expected type is an error because the integer width is unknown. Immutability, initializer type compatibility, effects, explicit moves, and consumption obligations are the same as for single `@let`. Trapping, throwing, or suspending initializers follow nested `@let` order and existing exit cleanup. This does not bind Wasm multi-value results. Use `(@let ($first $second) subject body)` to destructure pairs.

`just test-standard` checks standard types/functions/aliases/visibility and Future/Result/Option ABIs for Core+JSPI and Component.

`$task<T>` is a child task within the same instance. Create it with `@task.spawn` and consume it through explicit moves and join/drop. Execution targets Core+JSPI or Component. See [writers and child tasks](scoped-tasks.md).
