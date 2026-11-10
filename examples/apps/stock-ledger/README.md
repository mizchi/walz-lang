# Checking Improvements with an Inventory Ledger

English | [日本語](README.ja.md)

This implements processing different from the existing log aggregator, using new libraries, macros, and opaque types.
It updates inventory with Unicode keys and streams operation results as JSON Lines. Parsing, hash tables, checks, and JSON construction run in walz.
The initial implementation added no language syntax, compiler instructions, or runtime APIs.
The current main uses omitted type-argument shorthand. The [line-counting version](main-counted.walz) also uses type aliases and pair-destructuring let, separating line processing into `$advance`.
The [evaluation after syntax improvements](../../../docs/readability/implemented/REPORT.md) records changes and validation.

## Running

Use `walz-lang` as the working directory.

```sh
just build-stock-ledger
just stock-ledger                     # sample.tsv の結果を出力
just stock-ledger input.tsv
node runtime/text-cli.mjs target/stock-ledger < input.tsv > events.jsonl
just test-stock-ledger
just audit-stock-ledger
```

Input has the following format. Fields are TAB-separated, and commands must be uppercase.

```text
ADD<TAB>品目名<TAB>数量
TAKE<TAB>品目名<TAB>数量
SHOW<TAB>品目名
```

Item names are nonempty and at most 256 UTF-8 bytes. They use exact byte comparison and support quotation marks, backslashes, NUL, Japanese text, and emoji.
Quantities are ASCII decimal integers from 0..4294967295, allowing leading zeros. Internal inventory quantities use unsigned 64-bit arithmetic.
ADD increases quantity, TAKE decreases it, and SHOW returns the current value. Unregistered items have quantity 0. ADD 0 registers an item; TAKE 0 and SHOW on unregistered items do not.
Empty lines and lines beginning with `#` are skipped. Invalid syntax, insufficient stock, quantity overflow, and the item-count limit produce error records and continue to the next line.

Example output:

```json
{"op":"ADD","key":"日本語🦀","ok":true,"quantity":10}
{"op":"TAKE","key":"日本語🦀","ok":true,"quantity":7}
{"op":"TAKE","key":"日本語🦀","ok":false,"error":"insufficient-stock"}
```

Expected output for the entire sample is [expected.json](expected.json). Successful reads can still contain business errors; the CLI exits with code 0 on normal EOF.
Invalid UTF-8 and read/write failures terminate the program through main's Result err, and the CLI returns 1. Cancellation acknowledgement uses the existing task scope.
A write failure can leave partial output. Insufficient stock, overflow, and capacity errors leave inventory unchanged.

This ledger keeps state during one execution and uses no persistent database. Its concrete implementation has 64 buckets and at most 4,096 items.
State retained by GC is proportional to the item count. Reading JSON u64 numbers with ordinary JavaScript JSON.parse requires care with integer precision above 2^53-1.

## Two Ways to Write the Same Processing

- [main.walz](main.walz): Uses result.let and stream.fold-result.
- [main-explicit.walz](main-explicit.walz): Expresses the same processing with loop/match/close.
- [main-counted.walz](main-counted.walz): Produces the same output but changes the success value to the delivered line count. This includes empty lines, comments, and business-error lines whose I/O succeeds.

Build the line-counting version with `just build-core-js examples/apps/stock-ledger/main-counted.walz target/counted-ledger`.
Text-cli does not display the success payload. To inspect the number, use the runTextProgram example in the [cheatsheet](../../../CHEATSHEET.md#確認コマンド).

Both versions share the following modules, keeping the comparison focused on main's control flow.

| File | Responsibility |
|---|---|
| [parse.walz](parse.walz) | TSV, command enums, and quantity/key validation |
| [stock.walz](stock.walz) | Opaque State, a GC chained hash table, quantity updates, and business errors |
| [report.walz](report.walz) | JSON-escapes strings from input and outputs results |
| [program.walz](program.walz) | Connects business Results with I/O Results and loads shared libraries |

`Aa` and `BB` actually have the same hash, testing that collisions retain separate inventory entries.
Reusing `$json.object-builder.string`, `$string.concat`, and `$string.u64` made it possible to output variable-length input keys and quantities without constructing byte sequences manually.

## Improvements That Worked as Intended

The following numbers and execution costs describe the initial snapshot with explicit type arguments. They are not measurements of the current shorthand or line-counting version.
Main comparison after applying the same initial formatter:

| Metric | Handwritten version | Macro version |
|---|---:|---:|
| Lines | 70 | 31 |
| Tokens | 352 | 126 |
| S-expressions | 90 | 31 |
| Explicit match | 4 | 0 |
| Explicit stream.close | 3 | 0 |

This compares main, not the reduction for the whole program including the hash table, parser, and shared libraries.
Cleanup was not simply deleted; after macro expansion, it undergoes type, effect, and ownership checking.
Both versions were run with read/write failures and cancellation while waiting for either operation; endpoint registrations and ABI buffers were zero after completion.

External struct.set on inventory state and calls to update functions from pure functions were rejected at compile time.
The computational stock.add/stock.take/parse.command functions have no host or suspends effects; only app.process connects I/O.
The formatter was applied to both versions and every module, followed by fmt-check and execution tests.

## Comparing Behavior

Seven native tests covered collisions, Japanese keys, insufficient stock, capacity limits, zero operations, u64 overflow, and parsing.
Node builds all three versions for both Core and Component, comparing 3,000 operations against an independent JavaScript Map/BigInt implementation. The line-counting version also compares its success payload against delivered line count.
Additional checks cover empty input, the u32 maximum, leading zeros, quotation marks/backslashes/NUL, 256/257-byte keys, invalid fields, I/O failures, and cancellation.
Callers with a small test capacity and the u64 maximum also check quantity-overflow/capacity JSON.
Chromium executes actual input, updates, JSON output, and release on both backends too.

## Similar Execution Costs, with Temporary Results Still an Issue

2026-11-04, Apple M5 / macOS arm64, Node v24.21.0.
Core+JSPI delivered 3,000 lines from memory; after four warm-ups, each version was measured nine times in alternating order.
Measurement excludes Wasm compilation, instantiation, and input generation and runs until main completes. Output is checked every time against SHA-256 derived from independent expectations.

| Condition | Handwritten, remeasured | Macro, remeasured |
|---|---:|---:|
| ASCII keys | 12.34 ms | 12.23 ms |
| Keys with Japanese text, quotation marks, backslashes, and emoji | 13.19 ms | 13.19 ms |

Initial measurements were 12.15/12.05 ms for ASCII and 12.48/13.15 ms for Japanese; differences were unstable. These results do not support a speedup claim.
Every case had the same 2,842 outputs and 5,843 JSPI reads in both versions. After completion, linear ABI memory was 64 KiB with zero outstanding blocks.
This is not total Wasm GC heap usage. File-I/O throughput was not measured either.

Generated Core Wasm was 11,351 bytes for the handwritten version and 11,617 bytes for the macro version.
After expansion, main's Match sites increased from 4→6 and VariantNew sites from 3→7. These are static IR construction-site counts, not runtime allocation counts.
Source repetition decreased, but temporary Result construction and repeated branching remained. Simplifying immediately matched values is a candidate for subsequent optimization.

## Remaining Writing Difficulties

TAB search, range slicing, and integer parsing in the parser still required low-level loops. The hash table also needed a concrete implementation.
Useful next additions would be string/bytes search, splitting, integer parsing, and libraries of collections with fixed types.
This new application worked within existing specifications, so those features can first be organized as libraries.

Remeasurements are saved in `target/stock-ledger-audit/report.json`, with initial measurements in `report-initial.json`.
Artifacts, ir.json, sources, and shared libraries for both versions are saved in the same directory.
