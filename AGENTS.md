# Developing Walz

Respond to the user in Japanese.

- Use exploration → Red → Green → Refactoring. Demonstrate the changed behavior before implementing it.
- Put project tasks in `justfile`. Use Node.js 24+, pnpm, and Playwright for browser checks.
- Separate state, logic, and public API/type contracts. Prefer a small language and straightforward static analysis.
- Preserve standard WAT acceptance and Wasm type/exception/GC semantics. Do not bypass type, effect, ownership, or proof checks for optimization.
- If a Lean contract changes, update its producer, model, and checks together.
- Distinguish GC objects, linear ABI buffers, and external host resources; test normal, exceptional, and cancelled cleanup paths.
- Exercise runtime changes with both Core＋JSPI and Component; browser changes also need E2E checks.
- Write maintained docs in English. Keep English/Japanese README files, cheat sheets, and specifications consistent. See [the language policy](docs/development.md#documentation-language).
- Preserve recorded measurements, results, and input provenance. Harness names and source-path handling may be updated to keep historical workflows runnable; record adaptations rather than presenting a new run as the original measurement. Change the suite version and create a fresh run when a task or reference input changes.
- For Walz application source, use the [local Walz skill](.agents/skills/walz/SKILL.md); it is separate from compiler implementation guidance.

Main checks: `just check`, `just test`, `just test-language`, `just test-js`, `just test-e2e`, `just test-evals`, `just proof-check`, `just wit-check`, and `just test-docs`.
`just test-workflow` also checks GC reachability. Historical names and environment records are in [MIGRATION.md](MIGRATION.md).
The core module is implemented in `walz/core/` and exposed as `walz:core`.
