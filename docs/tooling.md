<a id="json診断と位置指定の型問い合わせ"></a>

# JSON diagnostics and position-based type queries

## check --json

```sh
just check-json examples/result.walz
cargo run --locked -- check --json path/to/main.walz
```

Emits one `walz-check-v1` JSON object to stdout. Performs the same verification as ordinary `check`, including proofs when laws are present. It does not execute host functions or guest programs.

```json
{"schema":"walz-check-v1","ok":true,"diagnostics":[]}
```

Failure returns `ok:false` and a `diagnostics` array. Each diagnostic has the same code/message/primary/related/notes/help fields as the [existing diagnostic contract](diagnostics.md). Primary and related positions map to original files. Source-loading, policy-loading, and external-tool failures also use JSON.

Success exits with 0; verification and I/O failures with 1. Invalid arguments exit with 2 and print usage to stderr. `--json` is a check option; use `walz check FILE` for the existing text output.

Adding `--warn-performance` includes optional performance diagnostics in `performance`.
Warnings are separate from `diagnostics` and do not cause failure on their own. For detected patterns, analysis scope, and improvement examples,
see [performance warnings for wasm-opt](wasm-opt.md#最適化しにくいソースへの任意警告).

## type-at

```sh
just type-at examples/result.walz 4 8
cargo run --locked -- type-at main.walz 7 12 --source lib.walz
```

The first file is the compilation unit to check. Without `--source`, queries use coordinates in that file. When specified, coordinates refer to a file loaded by that unit. Paths resolve from the current directory, as in the ordinary CLI.

Lines and columns are one-based; columns count Unicode scalars rather than UTF-8 bytes or display width. Positions are located in loaded source snapshots, excluding generated module wrappers from queries.
Source objects retained through the Rust API can query saved content even after original files change or are deleted. Bundled `walz:core` is a virtual module, not a loaded file reverse-mapped to an on-disk core-library path.

Success returns `walz-type-at-v1` JSON.

| Field | Contents |
| --- | --- |
| `mode` | `typed` for typed IR; `wat` for standard WAT/opaque paths |
| `matches` | Type candidates corresponding to the position; empty if none |
| `matches[].function` | Function/closure name written in the source |
| `specialization` | Generic function instantiation name; null for ordinary functions |
| `ty` | Checked, expanded type |
| `source_type` | Original explicit type spelling on a binding/result, such as `$Count`, `$T`, or `(@inst ...)`; null without an annotation |
| `location` | Label for the expression, binding, or result annotation (span/location/message) |
| `declaration` | Binding declaration position corresponding to a local reference/move; null if absent |
| `instantiations` | Array of Labels for positions requesting this instantiation |

Targets expressions, bindings such as arguments/captures/let/loop/case, and function result annotations. Multiple type instantiations of a generic body produce separate candidates. Type aliases and original type variables are retained for display; checking decisions use expanded types.

Comments, whitespace, and positions without corresponding expressions have empty matches. Standard WAT without typed IR returns `mode:wat` and an empty array, without guessing expression types. Unloaded files and out-of-range coordinates produce `E_SOURCE_POSITION`.

Compilation errors return `ok:false` and `diagnostics`, exiting with 1. Zero coordinates and missing arguments exit with 2. Type queries share existing type/effect/ownership checks and, like `types`, do not prove laws.

The Rust API is `inspect::type_at(&loaded, source_path, line, column, &policy)`. `typing::type_at` remains an API using byte positions in expanded strings. Editor/LSP communication and caches are outside this layer.
