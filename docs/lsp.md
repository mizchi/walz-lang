# Language Server Protocol

Start the server with `wlt lsp`. The transport is stdio JSON-RPC with `Content-Length` framing; stdout contains protocol messages only. Pass `--bindings generated/bindings.json` for a project with trusted host imports. The server never runs guest code or host imports.

The feature set is diagnostics, hover, completion, and go to definition for local bindings and named function/heap operands. Positions and incremental edit ranges use UTF-16, including surrogate pairs. Hover uses the checked type, including concrete generic instances. Native WAT has validator diagnostics; declarations can be shown as syntax without claiming an inferred expression type.

Open documents, including relative `%use` dependencies, are immutable in-memory overlays. Unsaved files need not exist on disk. A valid revision keeps its checked IR for queries; changes invalidate the cache and recheck open compilation units. Stale document versions are ignored. `didSave` and `workspace/didChangeWatchedFiles` refresh dependencies changed on disk. Diagnostics carry document versions; superseded notifications are suppressed by the transport.

`$/cancelRequest` is read independently of checking. Queued requests can be skipped and a cancelled result is replaced with error `-32800`; ongoing compiler work finishes within its existing limits. Cancellation does not preempt a Rust compiler pass. Headers, message bodies, queued messages, and open document counts are bounded.

Configure the editor's LSP client to launch command `wlt`, arguments `["lsp"]`, for `.wlt` files. With generated host bindings, use `["lsp", "--bindings", "generated/bindings.json"]`. The working directory determines relative policy paths.

The server retains at most eight checked snapshots. Open document text is shared between immutable revisions using `Arc<str>`; there are at most 128 documents, 16 MiB per document, and 64 MiB in total. Protocol headers are limited to 8 KiB and message bodies to 16 MiB, with eight queued messages. Closed documents and completed request IDs are retired rather than accumulated.

This server performs the type/effect/ownership checks used by `type-at`. Run `wlt check`, `wlt build`, and the proof commands for compilation and law verification. Rename, formatting requests, and cross-workspace indexing are outside this interface.

## Optional performance warnings

Set `initializationOptions: {"performanceWarnings": true}` when initializing, or send `workspace/didChangeConfiguration` with `settings: {"wlt": {"performanceWarnings": true}}`. The server advertises `experimental.wltPerformanceWarnings`. The default is disabled.

Warnings identify range copies in loop bodies, including checked helper calls, and GC allocations in loops that may escape to a host. They use severity 2, original UTF-16 ranges, and related locations for helper copies. They are conservative advice based on Binaryen 133, rather than measured hotspots or optimizer failure reports. Each checked revision computes them at most once; disabling them clears the published warnings. Invalid revisions keep strict errors and do not reuse prior advice. See [the rules and examples](wasm-opt.md#最適化しにくいソースへの任意警告).

## Editing incomplete or erroneous code

Diagnostics still come from the strict compiler. An error does not make the current document compilable. Alongside checked IR, the server indexes declaration syntax and lexical scopes from the current revision. When checking fails, hover can show **Declared syntax (unverified; no checked type is inferred)**, and definition can point to a declaration whose current position is known. This index is never supplied to code generation or the proof checker. No previous revision's inferred types or offsets are reused.

The editor-only prefix parser also keeps available declarations when an input ends inside a list. Broken tokens/comments can limit recovery to the preceding prefix. If an import cannot be resolved, recovery is limited to available local declarations; it does not guess foreign symbol identity or privacy. Implicit local bindings have no inferred type in this mode. Lexical shadowing is preserved. The index is bounded to 4,096 global symbols, 32,768 references/bindings, and 1,024 completion results.

Completion proposes visible declarations and explicit import selections. Inside `%use "wlt:core" ...`, it also lists exports from the compiler's embedded catalogue. It does not insert imports or change source automatically. Details from syntax are declarations, not evidence that a document has passed type/effect/ownership checks.

## User-defined syntax

[`%macro`](user-macros.md) declarations expose their patterns to hover and definition queries. Their explicit `bind` clauses supply lexical scope for completion and definition even when a current body fails checking. Type-directed rules with incompatible binding structures do not supply a guessed recovery scope. Checked argument types still come from the normal compiler.

The experimental `wlt/expandMacro` request uses hover-style `textDocument` and `position` parameters and returns `wlt-macro-expansion-v1`, including the selected rules, inferred types and source origins. It uses the current checked snapshot; an invalid revision returns no stale expansion. The server advertises `experimental.wltMacroExpansion` during initialization.

The result keeps the report structure and filters `expansions` to calls covering the requested position. For a whole-file report outside the editor, run `wlt macro-expand file.wlt` or `wlt macro-expand --text file.wlt`; see the [CLI guide](cli.md#マクロ展開と型の確認). Positions in the request use zero-based UTF-16, just like standard hover requests.

Macro expansion reports include `type_schema: "wlt-type-info-v1"`, a structured `types` map and each input's `usage` (`single`, `duplicated`, `discarded`). Counts describe syntax occurrences rather than runtime evaluations. Ordinary inferred generic calls use the same checked types and concrete specialization links as explicit calls; an ambiguous call produces `E_GENERIC_INFERENCE` rather than a guessed numeric type.

When parentheses are missing, recovery resolves current filesystem imports and unsaved library buffers using an editor-only delimiter completion snapshot. Completions and definitions include imported macros' declared single/repeated binders and preserve original source locations. This snapshot supplies only unverified syntax, never checked types or an expansion history. Changing/removing the imported macro invalidates those bindings immediately; incomplete strings/comments and invalid imports use conservative declaration recovery.
