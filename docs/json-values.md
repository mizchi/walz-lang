# JSON Values and Parser

`walz/core/json-value.walz` implements opaque immutable JSON values without adding language primitives.
Named imports from `walz:core` select only the required types and functions. Executable examples and a CLI are in [examples/json-parser](../examples/json-parser/README.md); the boundary fixture is `tests/component-fixtures/json-value.walz`.

Public JSON operations are grouped under `$json.*`. `$json-value` is an immutable JSON value, `$json-kind` is a kind enum,
and `$json-object` is the mutable object-writer type. Dots are part of WAT identifiers and do not implicitly introduce a `$json` value or type.
`$json.from-string` wraps the supplied string directly as a JSON string value; `$json.parse` parses JSON document syntax.

```wat
(module
  (@use "walz:core" (type $string)
    (func $json.from-string) (func $json.stringify))
  (func $quote (export "quote") (param $text $string) (result $string)
    (@effect reads writes allocates)
    (call $json.stringify (call $json.from-string $text))))
```

The output of `quote("null")` is `"null"`, including quotation marks. Passing `null` to `$json.parse` produces a JSON null value.
JSON string quoting and escaping are internal; there is no separate public quote function.

```wat
(module
  (@use "walz:core" (type $string) (type $result)
    (func $json.parse) (func $json.stringify))
  (func $normalize (export "normalize") (param $source $string)
    (result (@inst $result $string i32)) (@effect reads writes allocates)
    (@result.do
      ($value (call $json.parse (local.get $source)))
      (@result.ok (call $json.stringify (local.get $value))))))
```

| API | Contract |
| --- | --- |
| `parse(text)` | `result<(ref Value), i32>`; err is a UTF-8 byte position |
| `stringify(Value)` | JSON text, retaining numeric lexemes |
| `$json.from-string(text)` | Constructs a JSON string value without parsing |
| `$json.null/boolean/array/object` | Creates immutable values; array/object starts empty |
| `number(text)` | Validates and constructs a JSON number, returning Result |
| `prepend(array, item)` | Returns a new array with the item prepended, as Result |
| `set(object, key, item)` | Returns a new object with the item appended, as Result |
| `get/at` | Returns the value at a key/index as Option |
| `as-string/as-number/as-boolean` | Returns the corresponding payload as Option |
| `kind` | The null/boolean/number/string/array/object enum |

Numbers, including fractions and exponents, remain text. Integers such as `9007199254740993` are not rounded.
Combine with `string.parse-s64/parse-u64` for integer arithmetic.
Shortest binary-float formatting and correctly rounded decimal-to-floating-point conversion are outside this API's scope.

Objects retain duplicate keys; `get` returns the last value. Set does not overwrite existing values.
Sibling array elements and object fields are processed by loops; recursion is limited to value nesting.
The parser accepts at most 65,536 input bytes and 128 levels of array/object nesting. Empty containers also count toward depth. It accepts the four JSON whitespace characters and rejects trailing garbage,
control characters, malformed numbers, trailing commas, and malformed escapes.
To match Wasm UTF-8 semantics, it also rejects unpaired UTF-16 surrogate escapes.

Mutable object-writer operations are grouped under `$json.object-builder.*`: `new`, `string`, `u64`, `s64`, `boolean`, `object`, `null`, `number`, `value`, and `finish`.
Number leaves the builder unchanged if validation fails. Value can write any JSON value, including arrays.
No host-dependent numeric runtime is added.

Reversing the element list and field list was a possible implementation abstraction, but remains
two short loops with different payload shapes. Monomorphized generics were not added for this change; the choice will be reconsidered if duplication grows.


Numeric parsing chains integer, fraction, and exponent checks with `@result.do`.
String escapes, arrays/objects, and value dispatch use early returns to make success/failure exits explicit.
Internal Result/pair types use transparent aliases without introducing state or an additional runtime.
The parse return type and the semantics of immutable-value construction and lookup are preserved.

To append JSON to a general `$string-builder`, pass the result of `$json.stringify` to `$string-builder.text`.
The string/general builder does not depend on the JSON module. Internal `json-string.walz` is shared by JSON values and the object writer and is absent from the public `walz:core` catalogue.


See [JSON parser benchmarks](json-parser-bench.md) for parsing speed and comparisons including the ABI.
See [tuning results](json-parser-tuning.md) for measurements after improvements and experiments with SIMD and allocation strategies.
