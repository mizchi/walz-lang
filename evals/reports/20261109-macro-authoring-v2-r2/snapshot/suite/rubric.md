# Macro authoring rubric v2

Each axis is 0–4: 0 cannot complete; 1 needs implementation knowledge or a substantial workaround; 2 completes with recurring friction; 3 needs a small documentation lookup; 4 locally understandable and predictable. Cite frozen code, diagnostics, counters or executed results. No aggregate score. No point-delta comparison with v1's different authoring tasks.

- Consumer readability: follow success/failure paths; compare functions and both macro forms.
- Authoring: use inferred calls and public type queries without internal normalized syntax.
- Composition: combine binding DSLs, sequential dependencies and Result short-circuiting.
- Static guarantees: retain explicit scopes, duplicate-name rejection and checked runtime results.
- Diagnostics/editor: actionable explanations for failed tasks; use the paired v1 stdio LSP evidence without claiming new v2 editor coverage.
- Cost/scale: compare equal counts in one function vs many; include failures and deterministic work counters. No universal performance claims.
