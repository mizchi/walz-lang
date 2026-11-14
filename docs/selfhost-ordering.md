# Bootstrap GC type ordering

The modular bootstrap frontend orders concrete GC declarations before independent
validation and binary encoding. It supports at most 1,024 generated types and
65,536 dependency edges. The budgets live in `walz/frontend/limits.walz`; they
do not change the parser, field, function, instruction or rendering limits.

## Representation and contracts

- A name table indexes already resolved concrete type names. Structural hashing
  is followed by full key equality; collisions do not equate names or group keys.
- Only `ref` heap operands and `sub` parent operands add semantic dependency
  edges. A field binder matching another type name is not an edge.
- Producer-supplied explicit-rec keys and normalized arguments identify groups.
  A ring of edges connects each group's members, preserving the group even when
  its types do not reference one another. Two explicit groups, or an explicit
  group and external declarations, cannot merge into one inferred component.
- Sparse adjacency arrays grow geometrically up to the edge budget. There is no
  all-pairs reachability matrix. Type references are indexed before edges are
  collected, so forward references can be ordered.
- Iterative Tarjan uses explicit DFS and active-vertex stacks. Its SCC pass is
  O(V+E), and dependencies receive earlier component labels. A long dependency
  chain consumes bounded arrays rather than recursive Wasm calls.
- Inside a component, members retain source rank where possible. Stable sorting
  and parent-ready passes preserve the former inheritance ordering. A parent
  cycle is diagnostic 118. These member operations cost O(k²) for k members;
  the complete ordering pass is not claimed to be strictly linear.

Explicit single-member groups remain explicit. Implicit singletons retain the
ordinary standalone declaration spelling. Native type indices were resolved in
their original module before this pass; reordering declarations does not change
what an original numeric reference denotes. Subtype/layout validity, opaque
access and final instruction typing are still checked independently.

`graph.walz` contains bounded adjacency storage and SCC discovery;
`type-map.walz` contains name/group lookup; `type-order.walz` owns source/group
constraints and ordered output. No Rust or JavaScript compiler callback is used.

## Validation

```sh
just test-selfhost-order
just test-selfhost-profile
just test-selfhost-front
just test-selfhost-parity
just test-selfhost-e2e
```

The independent graph fixture runs in both Core+JSPI and Component. It checks
1,024-node chains/rings, generated small graphs against a separate reachability
oracle, invalid endpoints and edge budgets. The ordering fixture checks a
300-type forward dependency chain, mutual recursion, field-name collisions and
parent-cycle rejection. The full frontend corpus also checks explicit groups,
generic/nominal layouts, original indices and source diagnostics. Profiling tests
exercise layout reuse and large groups in Node and Chromium; ABI counters must
retire on successful and rejected calls.

The current full compiler source passes expansion and ordering. Compilation now
uses [direct syntax assembly](selfhost-syntax.md), bypassing intermediate WAT,
and passes syntax construction through same-origin atom sharing. Supported
[source ABI metadata](selfhost-abi.md) is now checked before specialization.
The v10 control/inference fixes resolve the later validation failures, and the
current subset passes [self-compiled stage equality](selfhost-stages.md).
See [profiling](selfhost-profiling.md) and
[remaining parity work](selfhost-parity.md).
