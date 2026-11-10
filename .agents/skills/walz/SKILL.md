---
name: walz
description: Read, write, debug, and review Walz programs, including .walz sources and .watz review notation, explicit core imports, GC data, effects, ownership, and CLI workflows. Use for Walz application work rather than unrelated Rust compiler changes.
---

# Walz programs

Walz extends WAT's S-expressions with Wasm GC data and checked high-level forms.
Use the installed `walz` compiler as the source of truth for syntax and contracts.
The compact [syntax reference](references/syntax.md) covers reading and writing;
the [CLI examples](references/cli.md) cover creating, inspecting, and running projects.

## Reading a program

- Identify the root module and its `@use` dependencies. `@use` selects names from a file; WAT `import` describes a host dependency. `@export` exposes a Walz source API; WAT `export` exposes the generated Wasm API.
- Read function parameters, results, `@effect`, and closure `@capture` declarations before following bodies. A missing result means unit, not an inferred return type.
- Distinguish shared GC references from owned box/Future/Stream/resource handles. `@move` consumes ownership; it does not make the payload's GC references exclusive.
- For checked types and original source positions, use `walz check --json`, `walz types`, or `walz type-at`. `walz expand` and `walz macro-expand --text` explain generated syntax without running host functions. Native WAT marked opaque has less detailed analysis.

Keep `.walz` S-expressions as the primary source. `walz print source.walz` prints Walz; pass `--format watz` for an optional view using `func`, `@export("name")`, and direct `match`/`if` expressions. The module wrapper is optional and bindings use immutable `let`. The same type/effect/ownership checks apply.

## Writing or changing code

- Preserve standard WAT instruction names. Use `@` on Walz extension entries, including `@let`, `@match`, `@closure`, and `@effect`. Fixed operators such as `+` and local/value shorthand are exceptions.
- Known Walz `@` entries are reserved and checked. Other WAT annotations are opaque; never place executable code inside metadata and expect it to run.
- Import core types and functions explicitly with `(@use "walz:core" ...)`. Local modules use `./` or `../` paths; filenames determine their scopes. No `@namespace` or wildcard import is needed.
- Declare function signatures. Bare numbers need an operand type or an expected type; add `(type i64)` to a let binding or use `i64.const` when ambiguous. There is no default integer width or implicit numeric conversion.
- A function body is one expression. Use `block` to sequence effects. `@let` is immutable; mutate explicit `mut` fields or pass the next loop state with tail `@recur`.
- Use `@case`, including `()` for an empty payload. Cover every enum/Option/Result variant; an optional final `(@default body)` covers remaining shared payloads without binding them. Owned/borrowed payloads require explicit `@case`. Application errors should be explicit Result values; traps do not become Result errors automatically.
- Declare every closure capture and supply its value at construction. Use `@inst` for explicit specialization; ordinary generic calls can infer only unambiguous type arguments.
- Consume each owned value on every reachable path, including early return, exceptions, and cancellation. Stream EOF still returns an endpoint that needs closing. Do not satisfy the checker by dropping owners or inventing implicit moves.
- Keep host effects and capabilities explicit. `pure` bounds semantic effects; it does not promise no traps, termination, or zero physical allocation. GC objects stay within an agent; threads share linear memory through explicit contracts.

## Validation

For changes to Walz source, check the affected root and run the relevant tests.
`walz test` discovers `tests/**/*.test.walz`; `walz bench --list` validates benchmarks without timing them.
Choose `--target core-js` when exercising JS values or external async hosts, and provide the required binding policy and host.
Do not introduce a new host dependency or bypass effect/ownership checks to make a sample run.
When diagnosing an error, keep its original code and source location and fix the violated contract first.

For a new application, `walz new app` supplies independent library, entry, test, and benchmark files.
Its local README and bilingual cheat sheets are additional references; this skill's bundled references also work when the skill is installed by itself.
