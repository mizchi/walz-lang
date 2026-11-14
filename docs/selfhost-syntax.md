# Direct syntax assembly

Project compilation and macro-query validation pass lowered trees directly to
the independent checker. They do not serialize escaped WAT and parse it again.
This avoids rejecting 360 KB of decoded UTF-8 export names merely because their
escaped text exceeds the debug printer's 1 MiB limit. Ordinary raw-source parsing
and the optional expansion/debug printer retain their existing limits.

## Construction and validation

`$node.new(kind, value, children)` creates an immutable, untrusted description.
`$syntax.assemble(nodes)` validates it before publishing opaque syntax:

- Kinds are atom (0), decoded byte string (1), or form (2).
- Atoms are nonempty valid UTF-8 tokens without WAT separators. Strings retain
  arbitrary decoded bytes; context-specific UTF-8/type requirements remain the
  checker's responsibility. Atoms/strings have no children; forms have no value.
- Every child index is nonnegative and strictly precedes its parent. Self,
  forward and out-of-range references are errors. The last node is the root.
- The arena has at most 262,144 nodes, depth 128, and 8 MiB of logical storage:
  decoded value bytes plus four bytes per child index. Storage checks use
  subtraction/division before addition, avoiding unsigned budget wraparound.
- Assembly produces immutable nodes with virtual ranges `[id, id + 1)`
  and an empty source byte buffer. Validated indexed nodes/arenas may be reused;
  other descriptions are normalized to fresh positions. Parsed syntax retains real UTF-8 ranges
  and its original source buffer. No caller-supplied range becomes a certificate.

Assembler errors use codes 3 (shape/token/reference), 7 (storage), 8 (node count),
9 (depth), or 100 (empty arena), with virtual positions. Syntax assembly is not
instruction/type/effect validation. Only `$checker.check` creates the opaque
module accepted by the binary emitter; a well-shaped unknown instruction still
fails that checker.

`walz/frontend/syntax.walz` converts lowered trees in postorder, normalizes the
internal ordinary-form marker, and retains an origin for every emitted node.
Its own early count/storage/depth guards stop construction on overflow with
diagnostic 150 and the current original source range. Checker diagnostics map
back through the virtual index to the original file and UTF-8 range, including
large decoded strings and imported definitions. The origin map and construction
state do not escape the compilation request.

Short atoms (at most 64 bytes) may share an index only when their complete byte
value, file, start and end match. Forms and decoded strings are not interned.
The request-local hash table grows geometrically, checks full equality after
hashing, and is bounded by the node budget. Sharing does not evaluate or merge
instructions or nominal declarations. Every logical visit, including a cache
hit, consumes a separate 1,048,576-visit budget; node sharing cannot evade it.

`$node.indexed` supplies an untrusted virtual-index hint. The assembler still
checks kinds, bytes, edges, depth and storage, then verifies every position.
Only a fully valid indexed arena is reused. Wrong hints are normalized; malformed
structure and instructions remain errors. Atoms do not allocate child builders.

## Profiling and regression gates

Profiler v3 adds `syntaxNodes` and `syntaxStorage`; v4 adds `syntaxVisits` and
`syntaxAtomHits`. Storage is logical charged
storage, not a measurement of GC heap use. These counters describe the main
module arena; separate query-probe validations do not overwrite them.
`renderedBytes` is zero during normal
compilation. Phase 4 is `lower`, phase 5 is `syntax`; old artifact schemas retain
their original `lower-render` / `reparse` meanings. Limits 8/9 identify generated
nodes/storage; limit 10 identifies syntax visit work. Debug text output keeps
its separate rendering limit.

```sh
just test-selfhost-syntax
just test-selfhost-profile
just test-selfhost-parity
just test-selfhost
just test-selfhost-e2e
```

Core/Component tests check malformed arenas, exact node/storage/depth boundaries,
independent checker rejection, original error ranges, large export names,
macro-query validation and ABI-buffer retirement after success and rejection.
The v7 shared Rust/bootstrap corpus adds two successful large-string cases and
one rejected return-type mismatch. An unchanged-input comparison of 241 v6
frontend cases retained all generated binaries and diagnostics. Recorded scalar
benchmark inputs and earlier measurements remain unchanged.

The v3 complete-source probe reached the generated-node budget. The v8/v4
source instead completed syntax assembly with 240,882 nodes, 537,068 logical
visits and 296,186 atom hits (2,481,495 logical storage bytes). It has 702 function,
368 heap and 51 alias instances with 646,073 charged elaboration visits. The
independent checker then rejected the unsupported `@abi.type` record annotation
in `walz/parser/diagnostic.walz` (code 101). The annotation is not erased to make
the probe pass. Full self-compilation and stage 1/2 comparison were incomplete
at that revision.
See [the measured tuning loop](selfhost-tuning.md).

The v9 source passed [checked ABI metadata validation](selfhost-abi.md).
It created 747 function, 398 heap and 55 alias instances and assembled 248,369
nodes from 549,199 logical visits with 300,830 atom hits (2,546,497 logical storage
bytes). Its next rejection was code 118 in independent GC type validation,
attributed to a match in `frontend/modules.walz`. The v10 control/inference fixes
now allow [self-compiled stage equality](selfhost-stages.md) within the same
syntax bounds. Differently sized source probes are not a performance comparison.
