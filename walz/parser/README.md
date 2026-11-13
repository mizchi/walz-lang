# Walz source parser

English | [日本語](README.ja.md)

`parser.walz` is a UTF-8 S-expression parser written in Walz. It returns an opaque,
immutable syntax arena with decoded token bytes and original byte ranges.
`number.walz` and `string.walz` provide checked string and i32/i64 literal decoding. These are
bootstrap compiler parts, not the complete Rust parser or a Wasm validator.

See [contracts, supported syntax, limits and tests](../../docs/selfhost.md).
The [checker](../checker/README.md) validates syntax; the binary backend is in [`../codegen/`](../codegen/README.md).

```wat
(@use "./parser.walz" (func $parser.parse) (type $syntax) (type $diagnostic))
;; $parser.parse(bytes) -> $result<(ref $syntax), (ref $diagnostic)>
```

No host import is needed. Parsed syntax retains its input bytes intentionally;
drop syntax references to allow Wasm GC to reclaim them.
