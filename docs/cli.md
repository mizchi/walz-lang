<a id="cli-のインストールと使い方"></a>

# Installing and Using the CLI

See [Getting started](getting-started.md) to create your first project, the [Language guide](language-guide.md) for syntax,
and [Feature reference](features.md) for supported features and restrictions. This page describes individual commands.

Install with Cargo from Rust 1.99 or later.

```sh
cargo install --git https://github.com/mizchi/walz-lang --locked --bin walz
walz --version
```

For a local checkout, run the following at the repository root.

```sh
cargo install --path . --locked --bin walz
```

`walz` is installed in Cargo's install directory (usually `~/.cargo/bin`). Include it in `PATH`. Using Walz does not require a source checkout, Cargo, just, or pnpm. The core library, proof-kernel source, and benchmark driver are embedded in the binary.

| Command | Additional requirements |
| --- | --- |
| `walz build`, `check`, static analysis | None for ordinary compilation |
| `walz lsp` | Editor with an LSP client; [diagnostics, hover, and go-to-definition](lsp.md) |
| `walz test`, `run --target core` (default) | Wasm-GC-capable `wasmtime` on PATH |
| `walz run --target core-js` | JSPI-capable Node.js 24 or later |
| `walz bench` | Node.js 24 or later on PATH |
| `verify`, builds containing laws or `@race-free` | `lean +v4.34.1` through elan |

`test --list` and `bench --list` only type-check and list declarations; no execution runtime is needed.

For the repository's [Minimal project](../examples/cli/README.md), run `cd examples/cli`, then use `walz build`, `walz test`, and `walz bench` directly.

## new

`walz new <directory>` creates an application in a destination that does not exist. Use `walz new -- <directory>` when the path might otherwise be interpreted as an option. Existing files, directories, and other existing destinations are rejected rather than overwritten.

The command works from the installed binary without downloads and does not initialize Git. It creates:

- `src/main.walz`, the application entry point, and `src/math.walz`, a source library.
- `tests/math.test.walz` and `app_bench.walz` for tests and benchmarks.
- English and Japanese `README.md` / `README.ja.md` and `CHEATSHEET.md` / `CHEATSHEET.ja.md`.
- A local `.agents/skills/walz/` skill with self-contained reference files for coding agents.

For example, create a project with `walz new app`, then run `cd app`, `walz run`, `walz build`, `walz test`, and `walz bench`. Runtime requirements are the same as those listed above.

The library publishes its source API through `@export`; the application entry module selects it through `@use`. Source-library exports are not automatically added to the application's root Wasm ABI. See [Agent skills](agent-skills.md) for the generated local skill and its reference files.

## wit-bindgen

`walz wit-bindgen api.wit policy.json generated [--world guest]` generates typed
imports in `bindings.walz`, shared `types.walz`, implementation stubs in
`exports.walz`, and checked `bindings.json`. The policy grants explicit host
capabilities; `--bindings generated/bindings.json` selects it for checking/building.
See [WIT bindings](wit-bindings.md). Component-to-Component wiring uses `@connect`;
WIT code generation does not instantiate a Component.

## print

`walz print FILE.walz` writes formatted Walz S-expressions to stdout. Pass `--format watz` for an optional Watz review view using `func`, `@export("name")`, immutable `let`, and direct `match`/`if` expressions. `--format walz` is the default; `walz print FILE.watz` prints a saved view back as Walz.

Printing does not edit, type-check, expand macros, resolve imports, or optimize source. Use `check` for validation. `print` takes one file; `--` allows a filename starting with a dash. See [source printing](review-syntax.md) for the exact reversible mapping and tooling boundaries.

## build

```sh
walz build
walz build examples/closure.walz
walz build examples/closure.walz -o dist/closure.wasm
walz build ./my-project
walz build main.walz output.wasm
walz build main.walz --target core-js -o dist/app
walz build main.walz --target component
walz build main.walz --target component --wit-input api.wit --world guest
walz build examples/components/main.walz --wit  # root component selects the target
```

With no arguments, searches the current directory in order: `main.walz`, `src/main.walz`, `main.watz`, `src/main.watz`. It does not search configuration files or parent directories. If both exist, `main.walz` wins. Explicit inputs also accept ordinary WAT.

Default output is `target/<input name without extension>.wasm`. For example, `src/main.walz` becomes `target/main.wasm`, and `foo.test.walz` becomes `target/foo.test.wasm`. A directory input uses its own entry point and `target/` directory.

Specify output with `-o` / `--output`. Relative output paths are based on the current directory; parent directories are created too. The existing `walz build INPUT OUTPUT` form remains supported. Output is written only after type, effect, ownership, required proof, and Wasm checks pass. Overwriting source or included files is rejected, including symlink/hard-link aliases.

Pass custom host-import policies with `--bindings policy.json`. Anything after `--` is treated as a file path.

Default output is Core Wasm. Execute it with `walz run main.walz` or pass it to Wasmtime as follows.

```sh
wasmtime run -W gc=y,function-references=y,exceptions=y --invoke main target/main.wasm
```

`--target core-js` bundles `index.js`, Wasm, codecs, and runtime in `target/<name>-js/`. Execution after building needs no checkout or Cargo. Call it through `const {instantiate}=await import('./dist/app/index.js')`. Node/browser JSPI are supported.
`--target component` generates `target/<name>.component.wasm` and a manifest. For JS conversion, see development task `just build-js` using pinned jco. Old `component` / `core-js` subcommands remain for low-level generation. [Runtime](runtime.md).

## run

```sh
walz run
walz run main.walz --invoke add -- 40 2
walz run main.walz --target core-js --invoke main -- 40 '"2n"'
walz run main.walz --target core-js --stdio --input input.txt --output output.txt
walz run main.walz --target core-js --bindings generated/bindings.json --host host.mjs
walz run main.walz --exit-code
```

Entry discovery matches build. Default export is `main`; select another with `--invoke`. Anything after `--` is a program argument. Core uses Wasmtime numeric arguments; core-js interprets JSON values if the export takes parameters. BigInt uses `"42n"`. Exports without parameters do not receive command-line arguments as function arguments.

Core-js always also exposes original string arguments as `list<string>` through `walz:host/cli@0.1.0.arguments`. Explicitly enable standard input/output with `--stdio`. `--input` / `--output` select files instead of stdin/stdout. Stdio mode does not append return values to stdout.

The `--host` module default-exports an options object or `({args,signal}) => options`. Supply required host implementations through `imports` and `eventLoop`. Resources can also be released through host `Symbol.asyncDispose`. Normally, values print to stdout; `result.err` goes to stderr with exit code 1. `--exit-code` uses an integer result in 0–255 as the exit code.

Specify files with as many `--read-file name=host-path` / `--write-file name=host-path` options as needed. The guest can use only named files. Overwriting source/input with output is rejected, including file aliases. [WASI file adapters](wasi-files.md).

Interrupts cancel the guest through Node AbortSignal and wait for host/I/O cleanup. SIGINT / SIGTERM sent only to the Unix parent process are forwarded to the child; termination is awaited before temporary artifacts are deleted. Cancellation exits with code 130.

<a id="戻り値と実行先の選択"></a>

### Choosing Return Values and Execution Targets

| Purpose | Command | Stdout |
| --- | --- | --- |
| Core Wasm returning numeric values or unit | `walz run main.walz` | Wasmtime value display |
| Receive shared values such as string/list/Result in JS | `walz run main.walz --target core-js` | JSON-encoded values; strings include quotes |
| Write raw text to pipes/files | `walz run main.walz --target core-js --stdio` | Content written by explicit I/O imports, without appended return values |
| Embed in a host as a WIT interface | `walz build main.walz --target component --wit-input api.wit` | Generates a Component to execute in a host |

Core displays GC references as `<anyref>` or similar and does not automatically convert them to string contents. `--stdio` grants the standard-I/O capabilities requested by the program; it does not convert return values to raw text.
`@test` / `@bench` in the same file are excluded from the public ABI of ordinary JS/Component builds. Their types/effects/ownership are still checked; execution and measurement are selected with `walz test` / `walz bench`.

<a id="test-と-bench"></a>

## test and bench

```sh
walz test
walz test tests/programs --filter closure
walz test --list
walz test tests/example.test.walz --timeout-ms 1000

walz bench
walz bench examples --filter JSON --samples 20
walz bench --list
walz bench --json --samples 3 --iterations 100 --warmup-ms 0
```

By default, `test` recursively discovers `*.test.walz` under `tests/`; `bench` recursively discovers `*_bench.walz` below the current directory. Direct file inputs accept any name. Multiple paths are allowed; each file is included once.

Recursive discovery does not descend into `target`, `node_modules`, `.git`, or symlinks. Explicitly naming these directories/files can include them. Paths after `--` are not interpreted as options.

Each test/benchmark runs in its own process; other cases continue after failures, traps, or timeouts. Zero cases is a failure. See [Testing](testing.md) and [Benchmarking](benchmarking.md) for declarations.

<a id="help-と終了コード"></a>

## Help and Exit Codes

```sh
walz --help
walz build --help
walz test --help
walz bench --help
walz help build
walz --version
```

Exit codes are 0 for success, 1 for source/runtime-environment errors or failed tests/benchmarks, and 2 for invalid arguments.

Use `just test-cli` for CLI development and `just test-install` for package-install checks. The latter installs into temporary storage, removes the build source, and checks the three commands and failure exit codes from another directory. Ordinary installation is also available through `just install`.

<a id="マクロ展開と型の確認"></a>

## Inspecting Macro Expansions and Types

```sh
walz check --json examples/user-macros/main.walz
walz macro-expand examples/user-macros/main.walz
walz macro-expand --text examples/user-macros/main.walz
walz expand examples/user-macros/main.walz
```

`macro-expand` outputs type-checked expansion history without executing the guest. Default output is `walz-macro-expansion-v1` JSON containing declarations, expansion origins, inferred types, input usage, and `checking` work counts. `--text` displays origins, inferred types, and syntactic input occurrence counts in human-readable form. `expand` outputs final WAT with all extensions lowered.

`single` / `duplicated` / `discarded` classify syntactic input occurrences, not execution counts including loops/branches. `checking` counters also measure checking-budget work, not CPU time. See [User-defined macros](user-macros.md) for syntax choices and limits and [LSP](lsp.md) for type/definition/expansion queries while editing.

<a id="js公開abiの検査"></a>

### Checking the Public JS ABI

```sh
walz esm-metadata main.walz
walz esm-metadata main.walz --stdin < edited.walz
```

`esm-metadata` checks Core+JSPI types, contracts, and public ABI, returning `walz-esm-metadata-v1` JSON.
It includes parameter/result types, JS names, async status, export positions, and loaded files.
`--stdin` checks input as unsaved contents at the given path; relative dependencies resolve from that path. It changes neither source nor generated artifacts.
For host imports, specify `--bindings policy.json`. [Content Mapper](content-mapper.md) also uses this entry point.

<a id="corejspi-のテスト計測"></a>

## Core+JSPI Tests and Benchmarks

`walz test --target core-js --bindings policy.json --host host.mjs` and
`walz bench --target core-js --bindings policy.json --host host.mjs` execute asynchronous imports and child tasks.
Each case gets its own process, Wasm instance, and host factory; host cleanup is awaited after success, failure, or cancellation.
`--list` only type-checks and starts neither the host module nor Node.
Timeout requests cooperative cancellation; hosts still running after up to 5 seconds of cleanup grace are forcibly terminated and reported as failures.

Core+JSPI benchmarks measure end-to-end execution, awaiting calls in order. Measurements include codecs, JSPI, and waits; results are restricted to ABI-supported shared values or unit.
These differ from default `core` Wasm batch measurements; JSON `options.target` and `summary.runtime` retain the distinction.

<a id="実行期限"></a>

## Execution Deadlines

`walz run --target core-js --timeout-ms 1000` sets a deadline starting at guest start.
Compilation and host-factory initialization are excluded. Timeout exits with 124;
external cancellation exits with 130. Both await guest/host cleanup before exiting.
See [Scopes](scoped-tasks.md) for other calls on the same instance and Walz child tasks.
`test` / `bench` deadlines include case Node initialization; unresponsive providers are terminated after a 5-second cleanup grace period.
