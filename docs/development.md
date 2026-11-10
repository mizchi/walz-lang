# Developing Walz

See [getting started](getting-started.md) for application setup. This guide covers changing the compiler and runtime.

```sh
git clone https://github.com/mizchi/walz-lang.git
cd walz-lang
cargo build --locked --bin walz
just --list
```

Use Rust 1.99+ and just. JS/browser work uses Node.js 24+ and the pnpm version pinned in package.json.
Execution tests need Wasmtime with Wasm GC; WIT checks need wasm-tools; contracts need Lean through elan.
See [ci/tools.json](../ci/tools.json) and the [installer](../ci/install-tools.py) for pinned versions and CI setup.
Native async worlds use a [dedicated compatible runtime](native-worlds.md).

```sh
pnpm install --frozen-lockfile
pnpm exec playwright install chromium
```

## Select checks for the change

| Area | Task |
| --- | --- |
| Rust formatting, warnings, generated API docs | `just check` |
| Compiler regressions | `just test` |
| Walz execution tests | `just test-language` |
| Core＋JSPI / Component JS execution | `just test-js` |
| Browser execution | `just test-e2e` |
| Independent ownership/shared-region kernels | `just proof-check` |
| WIT contract round trips | `just wit-check` |
| Guide examples, paired cheat sheets, and links | `just test-docs` |
| Scaffold generation and portable skill examples | `just test-new`, `just test-skill` |
| Vite integration | `just test-vite` |
| Content Mapper checks and protocol | `just test-content-mapper` |
| Pinned TS7 CLI/LSP/watch host | `just test-content-mapper-tsgo`; also uses git and Go |
| Fixed evaluation harness | `just test-evals` |

First demonstrate the affected failure, then implement the change and verify it succeeds.
Runtime changes must exercise both JS backends; browser changes also need Playwright.
Regenerate the core API with `just stdlib-docs`; do not edit signatures in [stdlib-api.md](stdlib-api.md) by hand.
The source implementation is in [walz/core/](../walz/core/README.md), exposed as `walz:core` independently of its filesystem layout.

## Inspect, format, and execute

```sh
cargo run --locked -- check --json examples/result.walz
cargo run --locked -- analyze examples/result.walz
cargo run --locked -- ir examples/result.walz
cargo run --locked -- fmt examples/result.walz
just samples
just probes
```

The [example catalog](../examples/catalog.json) tracks expected results.
`just test-docs` separately extracts code from the introductory guides and validates it.
See [frontend performance](frontend-performance.md), [wasm-opt](wasm-opt.md), and [architecture](architecture.md).

## Documentation language

Write maintained documentation in English. Keep both English and Japanese entry references: `README.md`/`README.ja.md`, `CHEATSHEET.md`/`CHEATSHEET.ja.md`, and `SPEC.md`/`SPEC.ja.md`.
Update the paired reference when a language rule or example changes; their executable Walz blocks must agree.
Unicode test data and historical quotation may remain in their original language.
Keep historical evaluation inputs, rubrics, measurements, and results identifiable. Harness names and source-path handling can be adapted to keep workflows runnable on the current tree; record such adaptations and do not substitute new measurements for old results. Add a new suite/run when changing a task or reference input.

The [portable Walz skill](agent-skills.md) and [application template](../templates/app/README.md) are shipped in the installed compiler.
Keep their instructions and examples consistent with the CLI and public language contracts.

## Historical records

[evals](../evals/README.md), dated documentation snapshots, and benchmark captures are immutable evidence.
Add a new run rather than changing old measurements or source hashes.
[MIGRATION.md](../MIGRATION.md) preserves the original Wasmtime prototype and former name gcwat.
