<a id="診断のデータ契約"></a>

# Diagnostic data contract

`walz::Error::diagnostic()` returns `&walz::diagnostic::Diagnostic`.
Diagnostic data is separate from text rendering, so callers can obtain positions and categories without parsing display text.
It can also be converted to JSON with `serde_json::to_value(error.diagnostic())`.

| Field | Meaning |
|---|---|
| `code` | Machine-readable category |
| `message` | Description of the primary error |
| `primary` | Source range of the problem; `None` for failures such as external tools |
| `related` | Positions and descriptions of declarations, earlier moves, effect causes, and similar context |
| `notes` | Additional information without positions |
| `help` | Name candidates or suggested repair approaches, rather than automatic fixes |

Each label has `span`, `location`, and `message`.
`span` is a half-open range of UTF-8 byte offsets; `location.line/column` are one-based.
Columns count Unicode scalars, rather than bytes or display widths.

Positions returned by the string API's `check` / `compile` refer to that string, with `location.path` set to `None`.
When using include, check the expanded text from `source::load(path)` and pass errors to `loaded.diagnostic(error)`.
This maps the primary error and every related position to ranges, lines, and columns in their respective original files.
A mapped label's `span` then refers to the file in `location.path`.
Ranges crossing include boundaries are limited to the contiguous portion in the starting file.
The CLI and test runner use this mapping too.

| code | Diagnostic |
|---|---|
| `E_CALL_ARITY` | Callee and argument count; ordinary calls do not count the function name |
| `E_UNKNOWN_LOCAL` | Name absent from the current scope |
| `E_TYPE_MISMATCH` | Expected and actual types |
| `E_OWNERSHIP_MOVED` | Reuse of a consumed or unavailable owned value |
| `E_OWNERSHIP` | Ownership-contract state mismatch or an unconsumed value |
| `E_EFFECT_CONTRACT` | Operation exceeding the declared effect limit |
| `E_OPAQUE_REPRESENTATION` | Representation access from outside the owning file |
| `E_ALIAS_CYCLE` | Reference cycle among transparent type aliases; shows the ordered path and each reference position |
| `E_GENERIC_INFERENCE` | Ambiguous, unresolved, or conflicting type arguments for a generic call |
| `E_GENERIC_SYNTAX` | Malformed angle application: empty arguments, missing comma/closing `>`, or excessive nesting |
| `E_MACRO` / `E_MACRO_UNKNOWN` | Macro declaration/template syntax, or an undefined macro |
| `E_MACRO_MATCH` | No rule matches the syntax and type constraints |
| `E_MACRO_TYPE` / `E_MACRO_INFERENCE` | Type-query target mismatch, or a missing required concrete/expected type |
| `E_MACRO_BINDING` / `E_MACRO_HYGIENE` | Violation of declared binding relationships, duplication, delegation, or free local references in templates |
| `E_MACRO_LIMIT` | Expansion count, size, depth, or checking-work limit |
| `E_CHECK` | Other syntax, name-resolution, or checking errors |
| `E_TOOL` | Tool, I/O, or similar failure without a source range |

`@closure.call` counts the explicitly passed environment.
Diagnostics distinguish the one environment argument from ordinary arguments. Captures are not added to the count.
Up to three name candidates are selected only from the current scope. Capture guidance is shown only inside closures with no candidates.
When no close name exists, diagnostics show up to four available bindings, their types, and declaration positions. These are presented as guidance if an existing binding was intended, without asserting a replacement.

Expected types include their basis in numeric operands, callee parameters, or declared results.
When a failing operand and the surrounding expected type are the same numeric type, conditional guidance suggests checking the corresponding operation family and constant types. This applies only to actual instructions with matching types and arity; no implicit conversion occurs.
When an inner expression already explains the cause, the outer result or call is not added as a cause.
Ownership diagnostics show the binding declaration and earlier move; branches display up to three reachable positions.
Effect diagnostics show the declaration and required effects together, following up to eight call levels to the causing operation. Throws already handled by catch are not treated as causes.
This context is diagnostic data; it is not used for ownership proof contracts or effect permission decisions.

Macros distinguish input positions from expansion-rule positions. A nonterminal `...` points to that token and suggests trailing repetition or recursive rules. When a delegated body has been reconstructed, guidance suggests forwarding the whole body and moving the wrapper inside the delegate's scope.
If `E_UNKNOWN_LOCAL` occurs in a captured initializer and a caller binding with the same name can be confirmed, a note explains that initializers use the caller's scope. This note is not attached to unrelated undefined names, and code is not accepted through implicit capture. See also the [supported scope](user-macros.md#hygiene-evaluation-and-intentional-bindings).

Run regression tests with `cargo test --locked --test diagnostics`.
Actual message comprehension and repair quality are assessed separately in the [evaluation loop](../evals/diagnostics/README.md).
