# Specialization inventory and exact code sharing

The inventory separates source specializations from the Wasm implementations
they produce. It does not change source type identity or skip checks. The
existing instance cache already reuses equal declaration/argument requests.

## Reproduce an inventory

```sh
just build-selfhost-inventory target/sharing/compiler
just inventory-selfhost examples/selfhost/portable.walz target/sharing/inventory target/sharing/compiler
just share-wasm target/sharing/inventory/subject.wasm target/sharing/shared.wasm
just test-selfhost-sharing
```

Output directories and shared-output files must be new. A capture keeps source
bytes, labels/hashes, compiler artifact hashes, recorder hashes, validated Wasm
and `report.json`. It does not overwrite an earlier measurement.

The separate `inventoryProject` guest operation returns a checked result and
an ordered function manifest: declaration/generated identifiers, original file
and UTF-8 range, and normalized argument spellings. It uses request-local state
and publishes copied snapshots. On rejection the manifest may be partial; it
does not certify a successfully emitted module. The existing profile v4 counter
schema and the portable compiler's five-operation ABI remain unchanged.

The host joins a successful manifest to independently validated defined
functions in emission order and checks both counts. Imported indices remain a
separate prefix. Source names and declaration grouping are explanatory metadata;
they never authorize sharing. The Rust inventory tool parses Wasm, not Walz.
There is no host fallback for guest expansion or checking.

The inventory entry itself also compiles through the guest into validated Core
Wasm (`target/selfhost-sharing/inventory-self`, 778 functions, 488,693 bytes).
This checks the new entry's source/backend coverage; generating its Canonical
ABI adapter with the self-compiled compiler remains separate work.

Reports include physical signatures, instruction/body sizes, direct-call
dependencies, exports, address-taken/start boundaries, exact-body groups and
aggregates by source declaration. Code/type sizes are section payload sizes;
function body sizes include locals but exclude the outer body-size LEB. Raw
binary equality is deliberately conservative about declared function subtypes,
sharing attributes, local encoding and concrete heap/callee indices.

## Observed compiler distribution

Captures `target/selfhost-sharing/inventory-1` and `inventory-3` retain the
compiler before and after common-helper factoring, with the same instrumentation:

| Metric | Before | After |
| --- | ---: | ---: |
| Portable module bytes | 489,969 | 489,563 |
| Code payload bytes | 476,550 | 476,169 |
| Defined functions | 776 | 773 |
| GC types | 424 | 424 |
| Functions with generic arguments | 323 | 323 |
| Their combined body bytes | 16,521 | 16,521 |
| Exact-body groups | 4 | 1 |

All four original groups were ordinary functions, not different generic
instantiations: identifier checks, child count, hex digits and UTF-8 conversion.
The first three now use shared source helpers. The remaining 55-byte conversion
body crosses a private library boundary; no extra public API was introduced just
to remove that copy. The byte difference includes index/LEB changes, rather than
only the removed body lengths.

Generic bodies occupy about 3.5% of the code payload on this input. Meanwhile
`core.source`, which constructs the embedded core source bytes, accounts for
242,552 body bytes. That literal-construction cost must be distinguished from
generic duplication. This workload currently provides no evidence for a large
exact cross-type sharing opportunity. A broader dictionary design needs a
separate representation/operation analysis; exact matching does not establish
that no broader sharing is possible.

These are code-size observations, not runtime or compilation speedups. Inventory
support itself adds compiler code/types compared with the earlier compiler;
the helper comparison does not establish a net shrink across unrelated versions.
The source programs intentionally differ between these refactoring captures;
they are not matched-input timing runs.

## Exact sharing contract

`code_sharing::share_exact` is an opt-in postprocessor. It groups matching
physical signatures and byte-identical bodies, then merges only private,
non-address-taken, non-start functions within each group. Exports and references
in instructions, elements, tables and globals preserve distinct function
identity. Calls and function-index-bearing standard sections are remapped;
removed function/local/label debug names are filtered. GC types are retained.

Input and output both undergo independent Wasm validation. Unknown custom
sections are rejected when rewriting would occur, because their function-index
contracts are unavailable. No-op inputs retain their original bytes. The pass
is conservative and performs one grouping round; transitive call-graph
equivalence and mutually recursive group refinement remain future work.

Source constraints, opaque access, effects, ownership and ABI checks belong to
the producer and still run for every requested source specialization. Neither
source metadata nor identical output from another specialization bypasses them.
No erased carrier, dictionary runtime or new generics syntax is introduced.
Ordinary compilation does not enable this postprocessor.

Tests cover different signatures/callees/GC immediates, signed-zero bits, imports,
function references, public identity, index/name remapping, traps, rejected
constraints, UTF-8 origins, request isolation, both adapters and ABI retirement.
The stage gate and shared Rust corpus continue to validate the compiler itself.

## Pinned Binaryen comparison

```sh
just probe-selfhost-sharing target/stages/run/stage1.wasm target/sharing/probe /path/to/binaryen-133/bin/wasm-opt
```

The probe accepts the Core GC stage bridge and requires Binaryen 133. It retains
the original input, exact-sharing output, a Binaryen roundtrip without an
optimization pass, duplicate-function elimination and similar-function merging.
The roundtrip baseline distinguishes reader/writer/type canonicalization from
the requested sharing pass. These are separate transformations, not one combined
optimization pipeline.

Each candidate must validate, preserve export alias identity, and reproduce all
184 positive outputs and 148 original negative diagnostics. Generated case Wasm
must match the original compiler byte for byte and execute with the same value
or trap. Reports retain cases, commands, hashes, recorder snapshots and sizes.
No timing ratio is reported and no default pipeline change follows from this
size experiment alone.

The retained `target/selfhost-sharing/probe-2` uses stage SHA-256
`7aba0cf3c9cb8717f1f10de046853fb8d69f38282ed0dc288996aa517ee4eef5`.
Every candidate completes the 332-case gate and retains the export alias relation:

| Candidate | Module bytes | Code payload | Functions | GC types |
| --- | ---: | ---: | ---: | ---: |
| Original | 485,920 | 473,084 | 756 | 390 |
| Walz exact sharing | 485,505 | 472,677 | 752 | 390 |
| Binaryen roundtrip | 473,548 | 462,483 | 756 | 355 |
| Binaryen duplicate elimination | 472,589 | 461,570 | 737 | 355 |
| Binaryen similar-function merging | 472,852 | 461,641 | 771 | 355 |

The duplicate-elimination candidate saves another 959 bytes over Binaryen's
roundtrip; similar-function merging saves 696. The much larger raw-to-Binaryen
change includes ordinary reader/writer and type canonicalization and cannot be
attributed entirely to a sharing pass. Similar merging introduces helpers, so
function count can rise while total size falls. These are results for this
captured compiler; they are not a proof of arbitrary Binaryen transformations
preserving every source contract or evidence of runtime speedups.

See [the ordered follow-up](../TODO.md#self-hosting-follow-up-in-implementation-order)
and [the implementation comparison](<../reports/Walz 単相化 共有方式 比較.md>).
