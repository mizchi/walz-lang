default: run

install:
    cargo install --locked --path . --bin walz

test-cli:
    cargo test --locked --test cli --test cli_build --test cli_print --test cli_run --test cli_async --test cli_deadlines --test cli_targets --test runner --test benchmarks --test inspection --test lsp

print file="examples/review/main.walz" format="walz":
    cargo run --locked --quiet -- print --format {{quote(format)}} {{quote(file)}}

test-print:
    cargo test --locked --lib review::tests
    cargo test --locked --test cli_print --test match_arms --test match_dispatch

test-runtime-cli:
    cargo test --locked --test cli_run --test cli_async --test cli_deadlines --test cli_targets --test interface_types --test wit_values --test wit_world --test stream_batch --test wasm_bridge --test scoped_tasks --test resources --test local_cancellation --test native_async --test native_resources --test abi_limits
    node --expose-gc --test runtime/interface-types.test.mjs runtime/wit-values.test.mjs runtime/wit-world.test.mjs runtime/stream-batch.test.mjs runtime/deadlines.test.mjs runtime/wasi-filesystem.test.mjs runtime/channels.test.mjs runtime/scoped-tasks.test.mjs runtime/wasm-bridge.test.mjs runtime/local-cancellation.test.mjs runtime/native-async.test.mjs runtime/native-resources.test.mjs

test-composition:
    cargo test --locked --test connection_syntax --test composition --test composition_resources --test composition_generics --test composition_queries --test composition_arguments --test wit_world --test native_async --test native_resources
    node --expose-gc --test runtime/composition.test.mjs runtime/composition-resources.test.mjs runtime/composition-generics.test.mjs runtime/composition-queries.test.mjs runtime/composition-arguments.test.mjs runtime/jco-native-results.test.mjs runtime/native-async.test.mjs runtime/wit-world.test.mjs

test-composition-e2e:
    pnpm exec playwright test --config playwright.composition.config.mjs

test-install:
    node tools/test-installed-cli.mjs

playground-build:
    node tools/build-playground.mjs

playground: playground-build
    node tools/serve-playground.mjs

test-playground: playground-build
    node --test playground/samples.test.mjs
    cargo test --locked -p walz-web
    pnpm exec playwright test --config playwright.playground.config.mjs

run file="examples/closure.walz":
    cargo run --locked --quiet -- run {{quote(file)}}

wat file="examples/closure.walz":
    cargo run --locked --quiet -- wat {{quote(file)}}

expand file="examples/exceptions.walz":
    cargo run --locked --quiet -- expand {{quote(file)}}

analyze file="examples/closure.walz":
    cargo run --locked --quiet -- analyze {{quote(file)}}

ir file="examples/closure.walz":
    cargo run --locked --quiet -- ir {{quote(file)}}

contracts file="examples/ownership.walz":
    cargo run --locked --quiet -- contracts {{quote(file)}}

proof file="examples/verified-laws.walz":
    cargo run --locked --quiet -- proof {{quote(file)}}

proof-check:
    lean +v4.34.1 proof/Ownership.lean
    lean +v4.34.1 proof/Delivery.lean
    lean +v4.34.1 proof/Tasks.lean
    lean +v4.34.1 proof/Flow.lean
    lean +v4.34.1 proof/Ranges.lean
    lean +v4.34.1 proof/AtomicCells.lean

verify file="examples/verified-laws.walz":
    cargo run --locked --quiet -- verify {{quote(file)}}

run-verified file="examples/verified-laws.walz":
    cargo run --locked --quiet -- run-verified {{quote(file)}}

types file="examples/variants.walz":
    cargo run --locked --quiet -- types {{quote(file)}}

check-json file="examples/result.walz":
    cargo run --locked --quiet -- check --json {{quote(file)}}

type-at file line column:
    cargo run --locked --quiet -- type-at {{quote(file)}} {{quote(line)}} {{quote(column)}}

test-inspection:
    cargo test --locked --test inspection --test typing --test diagnostics

test-collections:
    cargo test --locked --test collections
    node --expose-gc --test runtime/collections.test.mjs

test-simd:
    cargo test --locked --lib simd
    cargo test --locked --test simd
    node --expose-gc --test runtime/simd.test.mjs

test-simd-kernels:
    cargo test --locked --test simd_kernels --test simd
    node --expose-gc --test runtime/simd-kernels.test.mjs runtime/simd-kernels-bench.test.mjs

test-simd-collections:
    cargo test --locked --test simd_collections
    node --expose-gc --test runtime/simd-collections.test.mjs

test-dictionary-column:
    cargo test --locked --test dictionary_column
    node --expose-gc --test runtime/dictionary-column.test.mjs runtime/dictionary-bench.test.mjs

bench-dictionary-column output:
    node --expose-gc runtime/dictionary-column-bench.mjs {{quote(output)}}

build-table output="target/table":
    node runtime/table/build.mjs {{quote(output)}}

test-table:
    cargo test --locked --test table
    node runtime/table/build.mjs target/table-tests
    WALZ_TABLE_KERNEL=target/table-tests node --expose-gc --test runtime/table.test.mjs runtime/table-transfer.test.mjs runtime/table-input.test.mjs runtime/table-materialize.test.mjs runtime/table-portable.test.mjs runtime/table-types.test.mjs runtime/table-expressions.test.mjs

bench-table output:
    node --expose-gc runtime/table-bench.mjs {{quote(output)}}

profile-table snapshot output filter="":
    node --expose-gc runtime/table-profile.mjs {{quote(snapshot)}} {{quote(output)}} {{quote(filter)}}
    node runtime/table-profile-analyze.mjs {{quote(output)}} {{quote(output + "/analysis.json")}}

test-table-profile:
    node --test runtime/table-profile.test.mjs

build-selfhost output="target/selfhost":
    node runtime/build-core.mjs examples/selfhost/portable.walz {{quote(output)}}

selfhost-compile input output compiler="target/selfhost":
    node runtime/selfhost.mjs {{quote(input)}} {{quote(output)}} {{quote(compiler)}}

selfhost-check input compiler="target/selfhost":
    node runtime/selfhost.mjs --check {{quote(input)}} {{quote(compiler)}}

test-selfhost:
    cargo test --locked --test selfhost --test selfhost_gc_parity
    node --expose-gc --test runtime/selfhost.test.mjs runtime/selfhost-checker.test.mjs runtime/selfhost-gc.test.mjs runtime/selfhost-front.test.mjs runtime/selfhost-parity.test.mjs runtime/selfhost-profile.test.mjs runtime/selfhost-order.test.mjs runtime/selfhost-syntax.test.mjs runtime/selfhost-tuning.test.mjs runtime/selfhost-abi.test.mjs runtime/selfhost-control.test.mjs runtime/selfhost-operators.test.mjs

build-selfhost-profile output="target/selfhost-profile/compiler":
    node runtime/build-core.mjs examples/selfhost/profile.walz {{quote(output)}}

selfhost-profile input output compiler="target/selfhost-profile/compiler":
    node runtime/selfhost-profile.mjs {{quote(input)}} {{quote(output)}} {{quote(compiler)}}

compare-selfhost-profile before after input output samples="9":
    node runtime/selfhost-profile-compare.mjs {{quote(before)}} {{quote(after)}} {{quote(input)}} {{quote(output)}} {{quote(samples)}}

test-selfhost-order:
    node --expose-gc --test runtime/selfhost-order.test.mjs

test-selfhost-syntax:
    node --expose-gc --test runtime/selfhost-syntax.test.mjs

selfhost-tuning-prepare compiler inputs:
    node runtime/selfhost-tuning.mjs prepare {{quote(compiler)}} {{quote(inputs)}}

bench-selfhost-tuning before after inputs output samples="9":
    node runtime/selfhost-tuning.mjs compare {{quote(before)}} {{quote(after)}} {{quote(inputs)}} {{quote(output)}} {{quote(samples)}}

test-selfhost-tuning:
    node --test runtime/selfhost-tuning.test.mjs

test-selfhost-abi:
    node --expose-gc --test runtime/selfhost-abi.test.mjs

test-selfhost-control:
    node --expose-gc --test runtime/selfhost-control.test.mjs

test-selfhost-operators:
    node --expose-gc --test runtime/selfhost-operators.test.mjs

# Fixed-point gate kept separate from the shorter frontend regression suite.
test-selfhost-stage:
    node --expose-gc --test runtime/selfhost-stage.test.mjs

verify-selfhost-stages output seed="target/selfhost-profile/compiler":
    node runtime/selfhost-stage.mjs {{quote(seed)}} {{quote(output)}}

test-selfhost-profile:
    node --expose-gc --test runtime/selfhost-profile.test.mjs

selfhost-core-source:
    node tools/selfhost/embed-core.mjs

test-selfhost-parity:
    cargo build --locked --bin walz
    node --expose-gc --test runtime/selfhost-parity.test.mjs

test-selfhost-front:
    node tools/selfhost/embed-core.mjs --check
    node --expose-gc --test runtime/selfhost-front.test.mjs

test-selfhost-gc:
    cargo test --locked --test selfhost --test selfhost_gc_parity
    node --expose-gc --test runtime/selfhost-gc.test.mjs

test-selfhost-e2e:
    pnpm exec playwright test --config playwright.selfhost.config.mjs

capture-selfhost output:
    node runtime/selfhost-bench/capture.mjs {{quote(output)}}

verify-selfhost capture:
    node runtime/selfhost-bench.mjs verify {{quote(capture)}}

bench-selfhost capture output samples="9":
    node --expose-gc runtime/selfhost-bench.mjs {{quote(capture)}} {{quote(output)}} {{quote(samples)}}

test-selfhost-bench:
    cargo test --locked --test selfhost_parity
    node --test runtime/selfhost-bench.test.mjs

build-playground-size output:
    node tools/playground-size.mjs build {{quote(output)}}

size-playground capture output:
    node tools/playground-size.mjs measure {{quote(capture)}} {{quote(output)}}

test-playground-size:
    cargo test --locked --manifest-path tools/wat-web/Cargo.toml
    cargo fmt --manifest-path tools/wat-web/Cargo.toml -- --check
    node --test runtime/playground-size.test.mjs

probe-table-wasm-opt snapshot output optimizer="target/wasm-opt-research/binaryen-version_133/bin/wasm-opt":
    node --expose-gc runtime/table-opt-probe.mjs {{quote(snapshot)}} {{quote(output)}} {{quote(optimizer)}}

bench-table-tuning baseline stages output:
    node --expose-gc runtime/table-tuning.mjs {{quote(baseline)}} {{quote(stages)}} {{quote(output)}}

bench-table-io baseline candidate output:
    node --expose-gc runtime/table-io-bench.mjs {{quote(baseline)}} {{quote(candidate)}} {{quote(output)}}

bench-simd-collections output:
    node --expose-gc runtime/simd-collections-bench.mjs {{quote(output)}}

simd-kernels-capture output="target/simd-kernels/current" jsimd="../jsimd":
    node runtime/simd-kernels-bench.mjs capture {{quote(output)}} {{quote(jsimd)}}

simd-kernels-bench snapshot="target/simd-kernels/current" rounds="9" sizes="16,1024,4096,16384,262144":
    node --expose-gc runtime/simd-kernels-bench.mjs run {{quote(snapshot)}} {{quote(rounds)}} {{quote(sizes)}}

simd-kernels-compare before after output rounds="9" sizes="4096,16384,262144":
    node --expose-gc runtime/simd-kernels-compare.mjs {{quote(before)}} {{quote(after)}} {{quote(output)}} {{quote(rounds)}} {{quote(sizes)}}

wit file="examples/value-types.walz":
    cargo run --locked --quiet -- wit {{quote(file)}}

validate file="examples/closure.walz":
    cargo run --locked --quiet -- check {{quote(file)}}

build file="examples/closure.walz" output="target/program.wasm":
    cargo run --locked --quiet -- build {{quote(file)}} {{quote(output)}}

component file="tests/component-fixtures/event-loop.walz" output="target/component.wasm":
    mkdir -p target
    cargo run --locked --quiet -- component {{quote(file)}} {{quote(output)}}

component-wat file="tests/component-fixtures/event-loop.walz":
    cargo run --locked --quiet -- component-wat {{quote(file)}}

build-js file="tests/component-fixtures/event-loop.walz" output="target/event-loop":
    node runtime/build.mjs {{quote(file)}} {{quote(output)}}

build-core-js file="tests/component-fixtures/event-loop.walz" output="target/core-event-loop":
    node runtime/build-core.mjs {{quote(file)}} {{quote(output)}}

build-text output="target/text-filter":
    node runtime/build-core.mjs examples/io/text-filter.walz {{quote(output)}}

text-filter program="target/text-filter":
    @node runtime/text-cli.mjs {{quote(program)}}

text-file input output program="target/text-filter":
    @node runtime/text-cli.mjs {{quote(program)}} --input {{quote(input)}} --output {{quote(output)}}

build-log-stats output="target/log-stats":
    node runtime/build-core.mjs examples/apps/log-stats/main.walz {{quote(output)}}

log-stats input="examples/apps/log-stats/sample.tsv" program="target/log-stats":
    @node runtime/text-cli.mjs {{quote(program)}} --input {{quote(input)}}

test-log-stats:
    cargo test --locked --test log_stats
    node --test runtime/log-stats.test.mjs

profile-log-stats output="target/profile/log-stats" rows="30000" rounds="100":
    node runtime/log-stats-profile.mjs {{quote(output)}} {{quote(rows)}} {{quote(rounds)}}

compare-log-stats before="target/profile/log-stats-before-12346" after="target/profile/log-stats-final":
    node runtime/log-stats-bench.mjs {{quote(before)}} {{quote(after)}}

build-stock-ledger output="target/stock-ledger":
    node runtime/build-core.mjs examples/apps/stock-ledger/main.walz {{quote(output)}}

stock-ledger input="examples/apps/stock-ledger/sample.tsv" program="target/stock-ledger":
    @node runtime/text-cli.mjs {{quote(program)}} --input {{quote(input)}}

test-stock-ledger:
    cargo test --locked --test stock_ledger
    node --test runtime/stock-ledger.test.mjs

build-workflow output="target/workflow":
    node runtime/build-core.mjs examples/apps/workflow/main.walz {{quote(output)}}

workflow input="examples/apps/workflow/sample.tsv" program="target/workflow":
    @node runtime/text-cli.mjs {{quote(program)}} --input {{quote(input)}}

# Native model tests, independent oracle, both JS backends, limits and cleanup.
test-workflow:
    cargo test --locked --test workflow
    node --expose-gc --test runtime/workflow.test.mjs

# Also send the full 65,795-line workload through both host stream adapters.
test-workflow-stress:
    WALZ_WORKFLOW_STRESS=1 node --expose-gc --test runtime/workflow.test.mjs

# Save production artifacts before changing sources; compare in alternating order.
workflow-capture output="target/workflow-tune/current":
    node runtime/workflow-bench.mjs capture {{quote(output)}}

workflow-profile snapshot="target/workflow-tune/current":
    node runtime/workflow-bench.mjs profile {{quote(snapshot)}}

workflow-compare before="target/workflow-tune/before" after="target/workflow-tune/after-final" rounds="3":
    node runtime/workflow-bench.mjs compare {{quote(before)}} {{quote(after)}} {{quote(rounds)}}

# Compile and execute every complete module printed in CHEATSHEET.md.
test-cheatsheet:
    cargo test --locked --test cheatsheet

test-docs:
    cargo test --locked --test documentation --test cheatsheet

test-new:
    cargo test --locked --test cli_new

test-skill:
    cargo test --locked --test skill

# Frozen inputs and isolated writer directories for a fresh evaluation round.
grammar-eval-prepare round:
    node runtime/grammar-eval-prepare.mjs {{quote(round)}}

grammar-eval-verify directory:
    node runtime/grammar-eval-verify.mjs verify {{quote(directory)}}

# Prepare a fresh, unbounded evaluation ID; outputs are ignored under evals/runs/.
eval-prepare id:
    node evals/run.mjs prepare {{quote(id)}}

# Capture real diagnostics, then evaluate messages and intent-preserving repairs.
eval-diagnostics-prepare id:
    node evals/run.mjs prepare {{quote(id)}} --suite diagnostics

eval-verify run:
    node evals/run.mjs verify {{quote(run)}}

eval-score run:
    node evals/run.mjs score {{quote(run)}}

eval-compare before after:
    node evals/run.mjs compare {{quote(before)}} {{quote(after)}}

# Compare draft struct spellings without changing the language implementation.
eval-struct-prepare output:
    node evals/struct-literals/run.mjs prepare {{quote(output)}}

# Use the verifier frozen with the packets; result output must be a new file.
eval-struct-verify run output:
    node {{quote(run + "/support/run.mjs")}} verify {{quote(run)}} {{quote(output)}}

test-evals:
    node --test --test-concurrency=1 evals/*.test.mjs

audit-stock-ledger output="target/stock-ledger-audit":
    node runtime/stock-ledger-audit.mjs {{quote(output)}}

test-io:
    cargo test --locked --test text_io
    node --expose-gc --test runtime/text-io.test.mjs runtime/text-program.test.mjs runtime/node-io.test.mjs

test-stdlib:
    cargo test --locked --test library
    node --expose-gc --test runtime/library.test.mjs

test-views:
    cargo test --locked --test views --test array_views
    node --expose-gc --test runtime/views.test.mjs runtime/array-views.test.mjs

test-flatbuffers:
    cargo run --locked --quiet -- test tests/flatbuffers.test.walz
    node --expose-gc --test runtime/flatbuffers.test.mjs runtime/flatbuffers-pipeline.test.mjs runtime/flatbuffers-bench.test.mjs runtime/flatbuffers-inspect.test.mjs runtime/component-bundle.test.mjs

test-packed-arrays:
    cargo test --locked --test packed_arrays
    node --test runtime/packed-arrays.test.mjs

test-byte-access:
    cargo test --locked --test byte_access
    node --expose-gc --test runtime/byte-access.test.mjs

test-utf8-views:
    cargo test --locked --test utf8_views
    node --expose-gc --test runtime/utf8-views.test.mjs

test-checked-integers:
    cargo test --locked --test checked_integers
    node --expose-gc --test runtime/checked-integers.test.mjs

test-range-optimization:
    cargo test --locked --test range_optimization
    lean +v4.34.1 proof/Ranges.lean
    node --expose-gc --test runtime/range-optimization.test.mjs

test-value-optimization:
    cargo test --locked --test value_optimization --test optimization
    node --expose-gc --test runtime/value-optimization.test.mjs

test-angle-applications:
    cargo test --locked --test angle_applications --test generics --test generic_bounds --test cli_print
    node --test runtime/angle-applications.test.mjs

# Experimental synchronous direct GC ABI; the stable CLI targets are unchanged.
build-js-native file="examples/flatbuffers/main.walz" output="target/js-native" mode="js-strings":
    node runtime/build-js-native.mjs {{quote(file)}} {{quote(output)}} {{quote(mode)}}

test-js-native:
    cargo test --locked --test js_native
    node --expose-gc --test runtime/js-native.test.mjs

test-js-native-e2e:
    pnpm exec playwright test --config playwright.js-native.config.mjs

js-native-capture output="target/js-native-bench/capture":
    node runtime/js-native-bench.mjs capture {{quote(output)}}

bench-js-native snapshot output:
    node --expose-gc runtime/js-native-bench.mjs run {{quote(snapshot)}} {{quote(output)}}

size-js-native snapshot output optimizer="target/wasm-opt-research/binaryen-version_133/bin/wasm-opt":
    node runtime/js-native-bench.mjs size {{quote(snapshot)}} {{quote(output)}} {{quote(optimizer)}}

flatbuffers-capture output="target/flatbuffers-bench/capture":
    node runtime/flatbuffers-bench.mjs capture {{quote(output)}}

bench-flatbuffers snapshot="target/flatbuffers-bench/capture" output="target/flatbuffers-bench/run-1" rounds="9":
    node --expose-gc runtime/flatbuffers-bench.mjs run {{quote(snapshot)}} {{quote(output)}} {{quote(rounds)}}

bench-flatbuffers-pipeline snapshot output rounds="7":
    node --expose-gc runtime/flatbuffers-bench.mjs pipeline {{quote(snapshot)}} {{quote(output)}} {{quote(rounds)}}

profile-flatbuffers snapshot output filter="":
    node --expose-gc runtime/flatbuffers-inspect.mjs profile {{quote(snapshot)}} {{quote(output)}} {{quote(filter)}}

size-flatbuffers snapshot output optimizer="target/wasm-opt-research/binaryen-version_133/bin/wasm-opt":
    node runtime/flatbuffers-size.mjs {{quote(snapshot)}} {{quote(output)}} {{quote(optimizer)}}

optimize-flatbuffers snapshot output optimizer="target/wasm-opt-research/binaryen-version_133/bin/wasm-opt" mode="O3":
    node runtime/flatbuffers-size.mjs optimize {{quote(snapshot)}} {{quote(output)}} {{quote(optimizer)}} {{quote(mode)}}

compare-flatbuffers before after output filter="":
    node --expose-gc runtime/flatbuffers-compare.mjs {{quote(before)}} {{quote(after)}} {{quote(output)}} {{quote(filter)}}

probe-flatbuffers-sync snapshot output:
    node runtime/flatbuffers-sync-probe.mjs {{quote(snapshot)}} {{quote(output)}}

test-naming:
    cargo test --locked --test naming --test wit --test wit_bindings --test standard --test surface

# Generate public standard signatures from the compiler's embedded catalogue.
stdlib-docs:
    cargo run --locked --quiet --example stdlib-docs

check-stdlib-docs:
    cargo run --locked --quiet --example stdlib-docs -- --check

test-stdlib-docs:
    cargo test --locked --example stdlib-docs

test-modules:
    cargo test --locked --test modules --test static_imports --test dce
    node --test runtime/modules.test.mjs

fuzz seed="42" cases="64":
    WALZ_FUZZ_SEED={{quote(seed)}} WALZ_FUZZ_CASES={{quote(cases)}} cargo test --locked --test differential

fuzz-replay file:
    WALZ_FUZZ_REPLAY={{quote(file)}} cargo test --locked --test differential seeded_programs

fuzz-cancel seed="42":
    WALZ_FUZZ_SEED={{quote(seed)}} node --expose-gc --test runtime/fuzz.test.mjs

stdlib-capture output="target/stdlib/before":
    node runtime/stdlib-bench.mjs capture {{quote(output)}}

stdlib-compare before="target/stdlib/before" after="target/stdlib/after" rounds="5":
    node runtime/stdlib-bench.mjs compare {{quote(before)}} {{quote(after)}} {{quote(rounds)}}

# Save a guest/runtime snapshot and a Firefox guest profile.
profile label="current" rounds="40":
    node runtime/profile.mjs capture {{quote("target/profile/" + label)}} {{quote(rounds)}}

# Re-measure the saved snapshots in alternating order, without profiling.
profile-compare before="before" after="after":
    node runtime/profile.mjs compare {{quote("target/profile/" + before)}} {{quote("target/profile/" + after)}}

test-core-js:
    WALZ_BACKEND=core node --test runtime/runtime.test.mjs runtime/tasks.test.mjs runtime/core.test.mjs

memory-core-js:
    node --expose-gc --test runtime/core.test.mjs
    WALZ_BACKEND=core node --expose-gc runtime/memory-audit.mjs

run-js output="target/event-loop" export="main" arg="1":
    node runtime/run.mjs {{quote(output)}} {{quote(export)}} {{quote(arg)}}

test-js:
    pnpm test

test-e2e:
    pnpm exec playwright test

test:
    cargo test --locked --workspace

test-language path="tests" filter="":
    cargo run --locked --quiet -- test {{quote(path)}} --filter {{quote(filter)}}

list-tests path="tests" filter="":
    cargo run --locked --quiet -- test {{quote(path)}} --filter {{quote(filter)}} --list

bench path="." filter="":
    cargo run --locked --release --quiet -- bench {{quote(path)}} --filter {{quote(filter)}}

list-benches path="." filter="":
    cargo run --locked --quiet -- bench {{quote(path)}} --filter {{quote(filter)}} --list

test-bench:
    cargo test --locked --test benchmarks --test runner --lib
    node --test runtime/bench-runner.test.mjs

# Freeze @bench Wasm before changing std. All outputs require fresh paths.
bench-capture source output:
    node runtime/bench-tune.mjs capture {{quote(source)}} {{quote(output)}}

bench-profile snapshot output filter="" iterations="1000000":
    node runtime/bench-tune.mjs profile {{quote(snapshot)}} {{quote(output)}} {{quote(filter)}} {{quote(iterations)}}

bench-compare before after output samples="15":
    node runtime/bench-tune.mjs compare {{quote(before)}} {{quote(after)}} {{quote(output)}} {{quote(samples)}}

test-std-tune:
    cargo test --locked --test stdlib_tuning --test library --test views
    node --expose-gc --test runtime/bench-tune.test.mjs runtime/stdlib-tuning.test.mjs

frontend-bench filter="":
    cargo bench --locked --bench frontend -- {{quote(filter)}}

frontend-save label filter="":
    node tools/frontend-bench.mjs save {{quote(label)}} {{quote(filter)}}

frontend-compare label filter="":
    node tools/frontend-bench.mjs compare {{quote(label)}} {{quote(filter)}}

frontend-profile id output seconds="5":
    node tools/frontend-profile.mjs {{quote(id)}} {{quote(output)}} {{quote(seconds)}}

test-frontend:
    cargo test --locked --lib standard::performance_contracts
    cargo test --locked --bench frontend
    cargo build --locked --bin walz
    node --test runtime/frontend-reference.test.mjs

wit-check:
    mkdir -p target
    wasm-tools component wit interfaces/async-contract.wit --wasm -o target/async-contract.wasm
    wasm-tools validate --features all target/async-contract.wasm
    wasm-tools component wit interfaces/async-contract.wit -o target/async-contract.expected.wit
    wasm-tools component wit target/async-contract.wasm -o target/async-contract.roundtrip.wit
    cmp target/async-contract.expected.wit target/async-contract.roundtrip.wit

samples:
    cargo test --locked --test cli runs_all_gc_examples -- --nocapture

probes:
    cargo test --locked --test examples capability_probes_record_reproducible_language_limits -- --nocapture

check: check-stdlib-docs
    cargo fmt --all -- --check
    cargo clippy --locked --workspace --all-targets -- -D warnings

fmt:
    cargo fmt

fmt-walz file="examples/apps/log-stats/main.walz":
    cargo run --locked --quiet -- fmt {{quote(file)}}

check-walz-format file="examples/apps/log-stats/main.walz":
    cargo run --locked --quiet -- fmt-check {{quote(file)}}

# Check exact ownership entries and GC reachability; then sample retained memory.
memory-check:
    node --expose-gc --test runtime/memory.test.mjs runtime/retention.test.mjs
    node --expose-gc runtime/memory-audit.mjs

memory-recent output="target/recent-memory-audit/current":
    node --expose-gc runtime/recent-memory-audit.mjs {{quote(output)}}

bind wit policy output:
    cargo run --locked --quiet -- bind {{quote(wit)}} {{quote(policy)}} {{quote(output)}}

test-bindings:
    cargo test --locked --test wit_bindings
    node --test runtime/wit-bindings.test.mjs

test-resources:
    cargo test --locked --test resources
    node --test runtime/resources.test.mjs

test-json:
    cargo test --locked --test json_values --test buffers
    node --test runtime/json-value.test.mjs runtime/json-parser.test.mjs runtime/json-parser-bench.test.mjs

# Named GC construction, independent brace/literal sugars, and both JS ABIs.
test-structs:
    cargo test --locked --test struct_construction --test formatting
    node --test runtime/struct-construction.test.mjs

# Expected-type atoms, explicit let annotations, and both JS ABIs.
test-expression-sugar:
    cargo test --locked --test expression_sugar --test struct_construction --test type_aliases --test surface --test inspection
    node --test runtime/struct-construction.test.mjs

# Sealed scalar classes, monomorphized constraints and portable operators.
test-typeclasses:
    cargo test --locked --lib typeclasses
    cargo test --locked --test typeclasses --test logical_bits --test arithmetic_classes --test generic_bounds --test expression_sugar
    node --test runtime/typeclasses.test.mjs

test-lists:
    cargo test --locked --test lists --test wit_bindings --test differential
    node --expose-gc --test runtime/lists.test.mjs runtime/list-bindings.test.mjs runtime/numeric-lists.test.mjs

test-generics:
    cargo test --locked --test generics --test generic_bounds
    cargo run --locked --quiet -- test examples/generic-lists.walz
    node --expose-gc --test runtime/generics.test.mjs

# Inspect explicit @ extension syntax and native WAT boundaries.
test-syntax:
    cargo test --locked --test surface

# Embedded standard modules, explicit data type imports and grouped lets.
test-standard:
    cargo test --locked --test standard --test surface
    node --expose-gc --test runtime/standard.test.mjs

# Small typed-core features and the scalar representation at both JS boundaries.
test-core-expressions:
    cargo test --locked --test scalar --test result_do --test newtypes --test control --test exception_ownership --test verified --test resources
    node --test runtime/language-core.test.mjs runtime/resources.test.mjs

# GC witnesses, exact ABI/resource counts, and retained heap for the five core extensions.
memory-core-expressions output="target/core-expressions-memory/current":
    node --expose-gc runtime/core-expressions-memory.mjs {{quote(output)}}

# The JSON parser and serializer execute in Walz; this adapter only handles I/O.
build-json-parser output="target/json-parser":
    node runtime/build-core.mjs examples/json-parser/main.walz {{quote(output)}}

json-parser program="target/json-parser":
    @node runtime/json-parser.mjs {{quote(program)}}

# Freeze parser binaries, native release reference, sources and input documents.
json-bench-capture output="target/json-parser-bench/current":
    node runtime/json-parser-bench.mjs capture {{quote(output)}}

# Compare only frozen artifacts; output must be a new directory.
json-bench snapshot="target/json-parser-bench/current" output="target/json-parser-bench/results" rounds="9":
    node --expose-gc runtime/json-parser-bench.mjs run {{quote(snapshot)}} {{quote(output)}} {{quote(rounds)}}

# Run frozen before/after binaries together with identical input and API checks.
json-bench-compare before after output rounds="9":
    node --expose-gc runtime/json-bench/compare.mjs {{quote(before)}} {{quote(after)}} {{quote(output)}} {{quote(rounds)}}

test-json-bench:
    node --test runtime/json-parser-bench.test.mjs

# Typed Wasm atomics, affine operands, host shared memory, and browser workers.
test-threads:
    cargo test --locked --test threads
    node --test runtime/threads.test.mjs
    pnpm exec playwright test runtime/e2e/threads.spec.mjs

# Conditional race-freedom certificates and their independent trace kernel.
test-atomic-cells:
    lean +v4.34.1 proof/AtomicCells.lean
    cargo test --locked --test race_free
    node --test runtime/atomic-cells.test.mjs
    pnpm exec playwright test runtime/e2e/atomic-cells.spec.mjs

# First generate the example WIT bindings as documented in examples/wasi-files.
bench-stream-file input samples="5":
    WALZ_BENCH_INPUT={{quote(input)}} cargo run --locked --quiet -- bench examples/wasi-files/io_bench.walz --target core-js --bindings examples/wasi-files/generated/bindings.json --host examples/wasi-files/bench-host.mjs --samples {{samples}} --iterations 1 --warmup-ms 0

# Memoized ABI graph limits and independently cancellable Walz children.
test-local-cancellation:
    cargo test --locked --test abi_limits --test local_cancellation --test scoped_tasks
    cargo build --locked --bin walz
    node --expose-gc --test runtime/local-cancellation.test.mjs runtime/scoped-tasks.test.mjs runtime/deadlines.test.mjs

# Stdio LSP framing, unsaved modules, incremental UTF-16 edits and IR caching.
test-lsp:
    cargo test --locked --lib lsp
    cargo test --locked --test lsp --test inspection --test modules

# Native Canonical ABI execution requires Wasmtime 50+ (WALZ_WASMTIME overrides PATH).
test-native-engine:
    cargo test --locked --test native_async -- --include-ignored

# Original WIT identities, standard async producers and typed resource handles.
test-native-worlds:
    cargo test --locked --test native_async --test native_resources --test wit_world --test wit_bindings
    cargo build --locked --bin walz
    node --expose-gc --test runtime/native-async.test.mjs runtime/native-resources.test.mjs
    pnpm exec playwright test runtime/e2e/native-world.spec.mjs

# End-to-end developer workflows; records observed failures as well as successes.
eval-usability id:
    cargo build --locked --bin walz
    node --expose-gc evals/usability/run.mjs {{quote(id)}}

# Production ABI roots, readable generics, editor recovery, and typed value sugar.
test-usability-fixes:
    cargo test --locked --test application_harness --test generic_diagnostics --test lsp --test string_sugar --test option_result --test inspection

test-value-sugar:
    cargo test --locked --test string_sugar --test option_result --test expression_sugar --test struct_construction
    cargo build --locked --bin walz
    node --expose-gc --test runtime/option-result.test.mjs
    pnpm exec playwright test runtime/e2e/option-result.spec.mjs

# Checked list traversal, field destructuring, and non-atomic field updates.
test-high-level:
    cargo test --locked --test high_level --test lsp --test struct_construction --test readable_syntax
    cargo build --locked --bin walz
    node --expose-gc --test runtime/high-level.test.mjs
    pnpm exec playwright test runtime/e2e/high-level.spec.mjs

# Succinct indexes, conditional readonly certificates, and joined Worker queries.
test-succinct:
    cargo test --locked --test readonly_regions --test race_free
    node --expose-gc --test runtime/succinct.test.mjs runtime/succinct-pool.test.mjs runtime/readonly-regions.test.mjs runtime/workers.test.mjs
    pnpm exec playwright test runtime/e2e/succinct.spec.mjs runtime/e2e/workers.spec.mjs

test-workers:
    node --expose-gc --test runtime/workers.test.mjs runtime/succinct-pool.test.mjs
    pnpm exec playwright test runtime/e2e/workers.spec.mjs

succinct-build output="target/succinct/library" backend="core":
    node runtime/succinct/build.mjs {{quote(output)}} {{quote(backend)}}

bench-succinct id reference="../jsimd":
    node --expose-gc --experimental-wasm-modules runtime/succinct/bench.mjs {{quote(id)}} {{quote(reference)}}

# Explicit binary constructors, native value ABI copies, and initialization.
test-wasm-use:
    cargo test --locked --lib wit_bindgen::projection_limits
    cargo test --locked --test wasm_use --test static_imports --test modules
    node --expose-gc --test runtime/wasm-use.test.mjs
    pnpm exec playwright test --config playwright.composition.config.mjs wasm-use.spec.mjs

# Hygienic syntax objects, inferred type queries, static imports and editor scopes.
test-user-macros:
    cargo test --locked --test user_macros --test user_macros_lsp --test lsp --test inspection --test generics --test static_imports --test modules --test surface
    node --expose-gc --test runtime/user-macros.test.mjs
    pnpm exec playwright test runtime/e2e/user-macros.spec.mjs

expand-macros file="examples/user-macros/main.walz":
    cargo run --locked --quiet -- macro-expand {{quote(file)}}

# Freeze a fresh application/oracle/editor/scaling trial; never replace a run.
eval-user-macros id:
    cargo build --locked --bin walz
    node --expose-gc evals/macro-usability/run.mjs {{quote(id)}}

# Call inference, stable type projections, usage reports and replayable macro fuzzing.
test-macro-tools:
    cargo test --locked --test generic_inference --test macro_type_info --test macro_usage --test macro_differential

fuzz-user-macros seed="42" cases="64":
    WALZ_MACRO_FUZZ_SEED={{quote(seed)}} WALZ_MACRO_FUZZ_CASES={{quote(cases)}} cargo test --locked --test macro_differential

fuzz-user-macros-replay file:
    WALZ_MACRO_FUZZ_REPLAY={{quote(file)}} cargo test --locked --test macro_differential seeded_user_macros

expand-macros-text file="examples/user-macros/main.walz":
    cargo run --locked --quiet -- macro-expand --text {{quote(file)}}

# Function-local elaboration, delegated/repeated binders, and incomplete imported DSLs.
test-macro-composition:
    cargo test --locked --test macro_composition --test macro_resume --test user_macros_lsp --test macro_differential
    cargo run --locked --quiet -- test examples/user-macros/bindings.walz
    node --expose-gc --test runtime/user-macros.test.mjs

# Frozen current-API authoring attempts and concentrated vs distributed expansion.
eval-macro-authoring id:
    cargo build --locked --bin walz
    node --expose-gc evals/macro-usability-v2/run.mjs {{quote(id)}}

# Compare fixed inputs under open-world Binaryen 133; never overwrite a run.
wasm-opt-capture id optimizer="wasm-opt":
    cargo build --locked --bin walz
    node tools/wasm-opt-capture.mjs {{quote(id)}} {{quote(optimizer)}}

test-wasm-opt optimizer="wasm-opt":
    WALZ_WASM_OPT={{quote(optimizer)}} cargo test --locked --test match_dispatch --test performance_warnings -- --include-ignored

wasm-opt-compare before after id:
    node tools/wasm-opt-compare.mjs {{quote(before)}} {{quote(after)}} {{quote(id)}}

test-performance-warnings:
    cargo test --locked --test performance_warnings --test inspection --test lsp --test diagnostics

check-performance file="examples/ast-interpreter.walz":
    cargo run --locked --quiet -- check --warn-performance {{quote(file)}}

test-vite:
    cargo build --locked --bin walz
    cargo test --locked --test esm_metadata
    node --test vite/*.test.mjs
    pnpm exec playwright test --config playwright.vite.config.mjs

vite:
    cargo build --locked --bin walz
    WALZ={{quote(justfile_directory() + "/target/debug/walz")}} pnpm dev:vite

build-vite:
    cargo build --locked --bin walz
    WALZ={{quote(justfile_directory() + "/target/debug/walz")}} pnpm build:vite
    pnpm exec tsc -p examples/vite/tsconfig.json

test-content-mapper:
    cargo build --locked --bin walz
    cargo test --locked --test esm_inspection
    node --test content-mapper/*.test.mjs

build-content-mapper-tsgo:
    python3 tools/build-content-mapper-tsgo.py

test-content-mapper-tsgo:
    just build-content-mapper-tsgo
    cargo build --locked --bin walz
    TSGO={{quote(justfile_directory() + "/target/content-mapper-tools/tsgo")}} node --test content-mapper/tsgo.test.mjs
