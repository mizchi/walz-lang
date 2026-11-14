# Walz implementation constraints and improvement plan

## What did self-hosting actually demonstrate about generic growth?

### Takeaway

Walz already reuses the same declaration instantiated with the same normalized type arguments. The next question is whether *different* source instantiations produce equivalent lowered implementations. Existing counters establish the scale of compilation, not the amount of redundant emitted code.

### Cited Findings

- The bootstrap instance key is declaration identity (`ref.eq`) plus normalized argument-tree equality. Cache hits return the existing instance before emitting another one. Signature, return-type and layout results also belong to each concrete instance. — [instance lookup](/Users/mz/ghq/github.com/mizchi/wlt-lang/walz/frontend/generics.walz:986), [signature cache](/Users/mz/ghq/github.com/mizchi/wlt-lang/walz/frontend/generics.walz:1395), [layout cache](/Users/mz/ghq/github.com/mizchi/wlt-lang/walz/frontend/generics.walz:2266)
- Rust uses a `(template index, normalized argument strings)` cache. On a miss it clones and substitutes the template, then adds work to a queue. It rejects changing-argument polymorphic recursion and enforces finite limits. — [Rust expander state](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/generics.rs:129), [cache and substitution](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/generics.rs:279), [work queue](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/generics.rs:526)
- The retained `tuning-clean-final/compiler.json` capture has 119,985 instance requests, 118,783 hits, and 1,202 unique computations: 749 function, 398 heap and 55 alias instances. It reports 248,612 syntax nodes and 549,597 logical syntax visits. These totals include nongeneric declarations and are not an inventory of duplicate Wasm bodies. — [capture](/Users/mz/ghq/github.com/mizchi/wlt-lang/target/selfhost-validation/tuning-clean-final/compiler.json)
- The stage proof emits 485,920-byte stage 1/2 and 487,168-byte portable 1/2 modules; each corresponding pair is byte-identical. These are module totals, not a breakdown of generic overhead. — [stage capture](/Users/mz/ghq/github.com/mizchi/wlt-lang/target/selfhost-validation/stages-verified/report.json)
- The latest quiet comparison preserves output bytes across six subjects, but the compiler medians are 10,589.77 → 10,790.48 ms across only three measured samples. Repeated runs did not establish stable overall improvement. — [tuning record](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/selfhost-tuning.md:110)

### Inferences

- Do not present all 749 functions as avoidable generic duplication. First produce an instance-by-template inventory separating ordinary functions, generic specializations, heap types, aliases, helper bodies and wrappers.
- The bootstrap instance lookup is a linear scan of a snapshot. Improving lookup can reduce compile time without reducing the number of emitted functions; it is a separate experiment from body sharing.
- A shared symbolic/typed IR template can reduce substitution and tree allocation while still emitting separate concrete Wasm bodies. This is useful even when runtime bodies cannot be shared.

### Gaps

- There is no saved breakdown of bytes and instructions by generic template, no count of lowered bodies equivalent across distinct type arguments, and no measured benefit from generic body merging. No new benchmark was run for this research.

## Which code-sharing mechanisms fit Walz while preserving its contracts?

### Takeaway

Keep complete source type/constraint/ownership/effect checking, then share only equivalent implementations. Start with measurement and exact lowered-body sharing. Shape dictionaries should be considered only after those results show useful remaining duplication.

### Cited Findings

- Source nominal specialization identity is deliberately retained alongside concrete names, including representation privacy. The bootstrap keeps access checks at source operations even when layout facts are cached. — [Rust identity map](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/generics.rs:90), [bootstrap access and layout](/Users/mz/ghq/github.com/mizchi/wlt-lang/walz/frontend/generics.walz:2235)
- Function constraints are checked at instantiation; ABI metadata is independently certified; macro queries and guards participate in expansion. A type parameter can therefore affect compilation without appearing in an arithmetic expression. — [constraints](/Users/mz/ghq/github.com/mizchi/wlt-lang/walz/frontend/generics.walz:900), [constraint check call](/Users/mz/ghq/github.com/mizchi/wlt-lang/walz/frontend/generics.walz:1055), [query validation](/Users/mz/ghq/github.com/mizchi/wlt-lang/walz/frontend/generics.walz:4654), [macro guards](/Users/mz/ghq/github.com/mizchi/wlt-lang/walz/frontend/generics.walz:4947), [ABI contract](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/selfhost-abi.md)
- Every generated instruction still passes an independent checker; unsupported safety or ABI contracts remain errors. — [pipeline](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/selfhost-frontend.md:62)
- Core Wasm `struct.get` is typed against a particular heap type and field storage type. Equal reference widths do not make arbitrary struct-reference signatures interchangeable. — [Core validation rules](https://webassembly.github.io/spec/core/valid/instructions.html#valid-struct-get)
- Binaryen already provides exact duplicate-function elimination, including source types that become identical after lowering. It also offers similar-function merging that extracts constant/callee differences into arguments and retains thunks, with a size-benefit check. These are distinct mechanisms. — [DuplicateFunctionElimination source](https://raw.githubusercontent.com/WebAssembly/binaryen/main/src/passes/DuplicateFunctionElimination.cpp), [MergeSimilarFunctions source](https://raw.githubusercontent.com/WebAssembly/binaryen/main/src/passes/MergeSimilarFunctions.cpp)
- Walz already has a pinned Binaryen 133 optional postprocessing/measurement workflow. Normal builds and Playground do not run it, and existing ordinary measurements preserve open-world references and traps. New Binaryen versions require separate captures. — [current workflow](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/wasm-opt.md:55), [version/provenance rule](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/wasm-opt.md:140)
- The JS API has an exported-function object cache keyed by function address. A raw-WAT compatibility implementation must consider host-observable identity when replacing separate exported or address-taken functions. — [JS API object caches](https://webassembly.github.io/spec/js-api/index.html#webassembly-js-object-caches)

### Inferences

Recommended staged plan, without adding source syntax:

1. **Inventory before optimizing.** Record per-template instance counts, lowered signature, instruction/node counts, emitted code bytes and dependency edges. Fingerprint bodies using local/label alpha-normalization while preserving concrete heap-type operands, operation semantics, signedness, nullability, effects and callee identity. Include an exact comparison after hashing. Keep hashes/provenance stable and version the corpus.
2. **Separate source checking from implementation identity.** Every requested `f<T>` still validates its complete normalized source type arguments, constraints, opacity, ownership and effects, and retains its source origin. Only an implementation key may omit irrelevant parameters. Do not globally erase source instance identities or reuse a successful validation result to skip another source contract.
3. **Begin with genuinely unused type parameters.** A parameter is irrelevant only if it affects neither lowered parameter/result/local/heap types, instructions, selected operators, typed macro queries/guards, ABI operations, effect/ownership contracts, nor any transitive callee implementation. Compute dependencies to a fixed point over recursive call/type dependencies, conservatively treating unknown macro/query behavior as dependent. Syntactic absence from the expression body is insufficient. This early decision can save expansion/checking work; late merging cannot recover that spent work.
4. **Measure existing late passes first.** Use a new pinned-version capture to compare exact duplicate elimination, and optionally size-oriented similar-function merging. Do not implement an equivalent complex pass in Walz before learning whether these bodies actually occur. If integrating an internal pass later, operate on checked lowered IR and revalidate after rewriting all calls, exports, tables, element segments and function references. Numeric source indices must already have been resolved before any remapping. Preserve per-source diagnostics and profiling attribution even when implementation bodies share.
5. **Keep boundaries and identities explicit.** The simplest first internal merge candidate is a private, non-address-taken function with exactly equal Wasm signature and body. Preserve distinct wrapper functions when exported/reference identity is observable. Never merge GC type identities simply because layouts match, and do not replace typed references with `anyref` as a default optimization. Mutable fields, casts/tests, packed storage, subtyping, opaque types and resource ownership all need their own proof obligations.
6. **Only then consider shape/dictionary sharing.** A checked source-specific wrapper could call a shared internal helper supplied with layout/operation metadata or typed callable dictionaries. Sharing is safe only when all operation and storage requirements match; static reference width is not enough. Account for cast/boxing, metadata, indirect calls, wrappers, lost inlining and weaker GC optimization, as well as size saved. Numeric/SIMD hot paths should keep direct specialization unless evidence favors sharing. Dictionaries should carry compile-time resolved operations, not introduce unbounded runtime trait lookup.

Suggested test and measurement contract:

- Positive: repeated same-type calls produce one implementation; truly unused generic parameters share; a transitive unused-parameter case shares after fixed-point analysis; different aliases preserve declaration identity; structurally identical lowered private bodies merge deterministically; mutual recursion converges.
- Negative/exclusion: equal-width i32/f32 operations remain distinct; signed/unsigned operations remain distinct; i32/i64 and SIMD lanes remain distinct; equal-shaped foreign opaque types cannot access each other's fields; nullability/casts, mutable storage, different constraints, macro type queries, callee dictionaries and ABI views are not erased. Invalid skipped branches still fail checking.
- Identity/remapping: numeric functions/types, exported functions, tables/elements, `ref.func`, host-observable GC identity and raw-WAT behavior stay correct; retain wrappers where necessary. Reference equality on ordinary GC values is not the same issue as JS function-wrapper identity.
- Performance: compare raw Wasm **code** and **type** sections, total and compressed module size, frontend/expansion/validation time, deterministic charged nodes/work, peak/live memory (separately from logical counters), engine compile/startup time and representative execution time. Keep frozen inputs and both artifacts. Require the complete existing 332-case stage gate and 175-case Rust gate, then add new targeted cases. A byte-changing optimizer requires behavioral parity, independent validation and stage determinism; it cannot claim equality to the old unoptimized bytes.

### Gaps

- The current report does not establish that Binaryen 133 enables every cited `main` pass by default. Pin and inspect the chosen version/options in the new experiment rather than extrapolating from upstream `main` source.
- It is not yet known whether the best gain comes from unused parameters, internal helper factoring, exact-body merging, data representation changes, or lookup costs. The inventory is the decision gate.

## How should WAT ambiguity and type-context inconsistencies be improved?

### Takeaway

Preserve the WAT grammar as authoritative, put ambiguous shorthand behind explicit extended forms, and specify one bidirectional type/context model shared by the Rust and bootstrap conformance suites. This can be done without adding a general inference engine or changing Wasm semantics.

### Cited Findings

- WAT permits a symbolic label on a structured control instruction, and folded S-expressions are an abbreviation of instruction sequences. — [WAT grammar](https://webassembly.github.io/spec/core/text/instructions.html#labels)
- Bootstrap currently uses syntactic heuristics: a bare name followed by `then` is taken as the condition, while a preceding result clause/explicit operand is treated as a label. The corpus separately exercises local-condition shorthand, numeric local references, labelled if and multiple-expression branches; the last three remain outside Rust's typed `@use` parity gate. — [heuristic](/Users/mz/ghq/github.com/mizchi/wlt-lang/walz/frontend/lower.walz:380), [regression cases](/Users/mz/ghq/github.com/mizchi/wlt-lang/runtime/selfhost-front/control.mjs:21)
- Current fixes distinguish unit-valued statements, i32 conditions and declared branch result types; explicit string constructors infer, while bare strings require an expected string type. General semantic-never handling remains incomplete in the bootstrap. — [current contract](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/selfhost-stages.md:81)
- Wasm validation treats `return`, unconditional branches and `unreachable` as stack-polymorphic. This does not permit arbitrary type errors after unreachable code: explicitly introduced incompatible operands still invalidate a sequence. — [Wasm validation](https://webassembly.github.io/spec/core/valid/instructions.html)

### Inferences

- For strict WAT compatibility, do not let local-name lookup reinterpret a syntactically valid WAT label. Use explicit `(local.get $ready)` in an ambiguous native control position, or a separately marked extension (for example a future `@if`). A concrete spelling should be decided once and implemented in both frontends; this is a proposal, not current syntax.
- Introduce a small context contract: `check(expr, expected)`, `synthesize(expr)` for forms whose types are determined locally, a statement-sequence rule, a label/result-vector environment, and a control outcome such as normal/diverges rather than pretending `return` is an ordinary unit value. Mirror Wasm's reachable/unreachable stack discipline. Keep float/integer width and bare string inference conservative.
- Make valid native WAT, valid extended forms, deliberately ambiguous forms and invalid skipped/unreachable operations separate shared fixtures, but require both implementations to converge on one supported semantic contract rather than permanently accepting drift.

### Gaps

- The self-hosted subset does not yet implement the complete WAT grammar, block parameters or multiple results. A complete vector-based control validator is follow-up implementation work; the current single-result restriction should remain explicit until then.

## How should compiler diagnostics retain useful origins across sharing and expansion?

### Takeaway

Retain the stable compact error code/range API while adding a bounded trace as optional diagnostic data. Treat original source identity independently from a shared implementation body.

### Cited Findings

- Bootstrap tree nodes retain file/start/end, but public project errors currently contain only file/code/start/end. A final stack mismatch may point to the enclosing function. — [tree and error representation](/Users/mz/ghq/github.com/mizchi/wlt-lang/walz/frontend/tree.walz:67), [API/range boundary](/Users/mz/ghq/github.com/mizchi/wlt-lang/docs/selfhost-frontend.md:57)
- Code 118 groups invalid and unsupported value/GC types, declarations and subtypes in the host message. — [host diagnostic text](/Users/mz/ghq/github.com/mizchi/wlt-lang/runtime/selfhost.mjs:73)
- Rust already records specialization requests, associates errors with original calls and rewrites generated concrete names to source applications. That is a reusable model rather than an entirely new diagnostic mechanism. — [Rust annotation](/Users/mz/ghq/github.com/mizchi/wlt-lang/src/generics.rs:101)

### Inferences

- Add bounded related-origin records for macro invocation/definition, generic application/declaration, lowered node and validation phase. Distinguish unsupported feature from invalid source and internal lowering invariant failures. Include expected/actual type where the checker knows both.
- Have the typed expander attach an origin ID, not a copied prose trace, to every node. Intern or share bounded trace tails; keep caller origins on edges even when implementation IR is shared. Do not attach all call sites to one shared body and then guess which source caused an error.
- Tests should reject the same malformed input in Rust/bootstrap, retain UTF-8 original positions through nested imported macros and generics, expose the original source type names, enforce trace-depth limits and leave success/error/cancelled ABI buffers retired. Host rendering can provide human-readable messages without placing formatting/all strings into the minimal Wasm compiler.

### Gaps

- The extended trace requires a versioned optional ABI or separate inspection API; preserving existing five-operation portable clients needs an explicit compatibility decision. No diagnostic ABI change is implemented in this research.
