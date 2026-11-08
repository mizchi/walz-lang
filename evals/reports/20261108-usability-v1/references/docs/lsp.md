# Language Server Protocol

Start the server with `wlt lsp`. The transport is stdio JSON-RPC with `Content-Length` framing; stdout contains protocol messages only. Pass `--bindings generated/bindings.json` for a project with trusted host imports. The server never runs guest code or host imports.

The initial feature set is diagnostics, hover, and go to definition for local bindings and named function/heap operands. Positions and incremental edit ranges use UTF-16, including surrogate pairs. Hover uses the checked type, including concrete generic instances. Native WAT has validator diagnostics but no expression-level hover.

Open documents, including relative `%use` dependencies, are immutable in-memory overlays. Unsaved files need not exist on disk. A revision keeps its checked IR for queries; changes invalidate the cache and recheck open compilation units. Stale document versions are ignored. `didSave` and `workspace/didChangeWatchedFiles` refresh dependencies changed on disk. Diagnostics carry document versions; superseded notifications are suppressed by the transport.

`$/cancelRequest` is read independently of checking. Queued requests can be skipped and a cancelled result is replaced with error `-32800`; ongoing compiler work finishes within its existing limits. Cancellation does not preempt a Rust compiler pass. Headers, message bodies, queued messages, and open document counts are bounded.

Configure the editor's LSP client to launch command `wlt`, arguments `["lsp"]`, for `.wlt` files. With generated host bindings, use `["lsp", "--bindings", "generated/bindings.json"]`. The working directory determines relative policy paths.

The server retains at most eight checked snapshots. Open document text is shared between immutable revisions using `Arc<str>`; there are at most 128 documents, 16 MiB per document, and 64 MiB in total. Protocol headers are limited to 8 KiB and message bodies to 16 MiB, with eight queued messages. Closed documents and completed request IDs are retired rather than accumulated.

This server performs the type/effect/ownership checks used by `type-at`. Run `wlt check`, `wlt build`, and the proof commands for compilation and law verification. Completion, rename, formatting requests, and cross-workspace indexing are outside this initial interface.
