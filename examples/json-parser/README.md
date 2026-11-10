# Running the Walz JSON Parser

English | [日本語](README.ja.md)

JSON parsing, construction of the GC-backed AST, and serialization all run in Walz.
The implementation is [walz/core/json-value.walz](../../walz/core/json-value.walz); the entry point is [main.walz](main.walz).
The Node [CLI](../../runtime/json-parser.mjs) handles UTF-8 input/output and diagnostic display without calling `JSON.parse`.

```sh
just build-json-parser
just json-parser < examples/json-parser/sample.json
```

Output:

```json
{"answer":42,"items":[null,true,"日本語🦀"],"large":9007199254740993,"decimal":1.2300e-2}
```

To pass a file as an argument:

```sh
node runtime/json-parser.mjs target/json-parser examples/json-parser/sample.json
```

Whitespace is removed, while key order, duplicate keys, and numeric lexemes are preserved. `9007199254740993` is not converted to floating point.
The parser supports null/boolean/number/string/array/object, Unicode escapes, and UTF-16 surrogate pairs.
It rejects unpaired surrogates, trailing commas, extra characters, malformed numbers, and malformed escapes.

`normalize(text)` returns `result<text,i32>`; err is a zero-based UTF-8 byte position.
For invalid JSON, the CLI exits with code 1, leaves standard output empty, and reports the position on standard error.
Displayed lines/columns are one-based; columns count Unicode scalar values. CRLF counts as one line break.
Success exits with code 0. Input is one document, which may span multiple lines. This is not an interface that reads JSON Lines individually.

```text
JSON parse error at byte 3 (line 1, column 4)
```

The example above is for `[1,]`. Input is limited to 65,536 bytes and array/object nesting to 128.
Empty containers count toward depth. The CLI stops reading when the limit is exceeded and also rejects invalid UTF-8.
A BOM is not silently removed; the parser treats it as an invalid leading character.

## Reading the Implementation

- `$parse-value`: Dispatches on the first byte. Early `return` expresses depth checks and each value's entry path.
- `$parse-number`: Reads the integer, fraction, and exponent parts in sequence with `@result.do`, passing each successful position to the next stage.
- `$string-end` / `$parse-string`: Scans ordinary string contents in batches and uses a builder only when escapes occur.
- `$escape-unicode`: Converts Unicode escapes to UTF-8.
- `$parse-array` / `$parse-object`: Reads sibling elements with loops; recursion is used only for nested values.
- `$parse`: Consumes final whitespace, checks that input is exhausted, and returns the AST.

The AST uses opaque variants and immutable GC lists, without host objects or a custom heap.
`$answer` in `main.walz` demonstrates inspecting the AST using only public `get`, `as-number`, and integer parsing.

```sh
just run examples/json-parser/main.walz  # 42
just test-json
```

Wasmtime executes boundary and Unicode checks; Node's Core+JSPI/Component backends compare against an independent JSON parser.
Tests also check CLI standard input/files/error display, zero outstanding ABI buffers after each call, and both Chromium backends.
This is a recursive-descent parser that reads all input through EOF at once, rather than an incremental streaming parser.


See [JSON parser benchmarks](../../docs/json-parser-bench.md) for speed comparisons. JSON.parse, serde_json release, parsing itself, input ABI conversion, and reserialization can be measured separately.
The [tuning results](../../docs/json-parser-tuning.md) record improvements and the SIMD approach that was not adopted.
