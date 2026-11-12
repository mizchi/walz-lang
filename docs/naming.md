<a id="標準名とwitの対応"></a>

# Standard names and their WIT mappings

Public standard library types use lowercase kebab-case; related functions generally use `$type-name.operation-name`.
JSON construction, parsing, serialization, and lookup are grouped under `$json.*`; mutable object writers use `$json.object-builder.*`.
`$json.from-string` constructs a JSON string value, `$json.parse` parses a document, and `$json.stringify` serializes a value.
In WAT, dots are part of identifiers rather than namespaces. Walz follows the same rule, explicitly binding file-level public names with `@use`.
This naming convention is not imposed on users' WAT identifiers.

| Public type name | Related function examples |
| --- | --- |
| `$bytes` | `$bytes.equal`, `$bytes.find` |
| `$string` | `$string.u64`, `$string.find` |
| `$bytes-view` | `$bytes-view.new`, `$bytes-view.slice` |
| `$string-view` | `$string-view.new`, `$string-view.to-string` |
| `$array-view` | `$array-view.new`, `$array-view.slice`, `$array-view.to-list` (instantiate the element type and source array type) |
| `$dict` | `$dict.new`, `$dict.get` |
| `$list-builder` | `$list-builder.push`, `$list-builder.finish` |
| `$string-builder` | `$string-builder.text`, `$string-builder.finish` |
| `$json-object` | `$json.object-builder.string`, `$json.object-builder.finish` |
| `$json-value` | `$json.parse`, `$json.stringify` |

Views, dictionaries, builders, and JSON values are opaque GC types, so write value types as `(ref $string-view)` or
`(ref (@inst $dict i64))`. Do not add `.View` / `.Dict` / `.Builder` suffixes.
See the [generated API list](stdlib-api.md) for public names.

<a id="witの組み込み型とwalz固有の型"></a>

## WIT built-in types and Walz-specific types

| Walz standard name | WIT representation and support |
| --- | --- |
| `$string` | `string`; internally validated UTF-8 |
| `$tuple<T,U>` | `tuple<T,U>`; currently limited to two elements |
| `$option<T>` / `$result<T,E>` | `option<T>` / `result<T,E>` |
| `$list<T>` | `list<T>`; elements restricted to shareable values |
| `$future<T>` / `$stream<T>` | `future<T>` / `stream<T>`; converted by existing async adapters |
| `i32` / `i64` / `f32` / `f64` | Current WIT generation uses `s32` / `s64` / `f32` / `f64` |
| `$bytes` | Dedicated Walz representation; `list<u8>` in WIT |
| `$unit` | Walz unit; represented by omitted results/payloads or existing unit adapters |
| `$box<T>` | Walz owned GC box; distinct from WIT `own<R>` |
| `$resource-handle<R>` / `$borrow<R>` | Walz external resources and borrows; require explicit host bindings |

WIT `own<R>` denotes resource ownership, so it is not used to name GC boxes.
GC boxes use `@box.new` and type `(@inst $box T)`. External resources use existing `@resource.*` operations.
Naming changes and ABI support are separate features. See [WIT bindings](wit-bindings.md) for current bool/char, integer, tuple, and variant support. WIT resource input/output comes later.
For example, adding `$u32` as a transparent i32 alias would not create a type with unsigned comparison/division semantics.
WAT i32/i64 operations and WIT boundary types are defined separately.
Numeric helper names such as `$i32.checked-add-u` are ordinary function identifiers.
The `-u` specifies unsigned bit-pattern semantics; the scalar type and its ABI remain i32/s32.
See [checked size arithmetic](checked-integers.md).

Built-in extension vocabulary likewise uses `@string.*`, `@tuple.*`, and `@box.*`.
Existing internal `text` / `pair` / `own` representations in typed IR, ownership checking, and ABI policies are retained.
Tuples can be destructured with `@let ($first $second) value body`; existing `case pair` / `case own` destructuring labels are retained too.

<a id="witへ出す名前"></a>

## Names exported to WIT

Walz's `$string-view.slice` is a function identifier. Specify lowercase kebab-case external names such as `"string-slice"` when exporting to WIT.
GC references are not WIT value types, so explicitly convert opaque GC types such as views to string/bytes or other supported types.
No rule automatically converts related functions into WIT resource methods or constructors.

```wat
(module
  (@use "walz:core" (type $string)
    (func $string-view.new) (func $string-view.slice) (func $string-view.to-string))
  (func $string-slice (export "string-slice")
    (param $source $string) (param $start i32) (param $length i32)
    (result $string) (@effect allocates)
    (call $string-view.to-string
      (call $string-view.slice (call $string-view.new $source) $start $length))))
```

This becomes `string-slice: func(p0: string, p1: s32, p2: s32) -> string` in WIT.
Inspect actual output with commands such as `just wit examples/views.walz`.

Names follow the [WIT identifier rules](https://component-model.bytecodealliance.org/design/wit.html#identifiers)
and [WIT type specification](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md).
WIT permits uppercase words too, but the Walz standard library consistently uses lowercase.

<a id="凍結済みの記録"></a>

## Frozen records

Old spellings are not accepted by normal compilation. Past evaluation and measurement sources remain unchanged and are read through the existing internal legacy loader.
Only public names at import sources are converted; old local names, comments, strings, and original diagnostic positions are preserved.
Current evaluation tasks receive a new version, with inputs fixed in a new run.
