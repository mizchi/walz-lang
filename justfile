default: run

run file="examples/closure.wlt":
    cargo run --locked --quiet -- run {{quote(file)}}

wat file="examples/closure.wlt":
    cargo run --locked --quiet -- wat {{quote(file)}}

expand file="examples/exceptions.wlt":
    cargo run --locked --quiet -- expand {{quote(file)}}

analyze file="examples/closure.wlt":
    cargo run --locked --quiet -- analyze {{quote(file)}}

ir file="examples/closure.wlt":
    cargo run --locked --quiet -- ir {{quote(file)}}

contracts file="examples/ownership.wlt":
    cargo run --locked --quiet -- contracts {{quote(file)}}

proof file="examples/verified-laws.wlt":
    cargo run --locked --quiet -- proof {{quote(file)}}

proof-check:
    lean +v4.34.1 proof/Ownership.lean
    lean +v4.34.1 proof/Delivery.lean
    lean +v4.34.1 proof/Tasks.lean
    lean +v4.34.1 proof/Flow.lean

verify file="examples/verified-laws.wlt":
    cargo run --locked --quiet -- verify {{quote(file)}}

run-verified file="examples/verified-laws.wlt":
    cargo run --locked --quiet -- run-verified {{quote(file)}}

types file="examples/variants.wlt":
    cargo run --locked --quiet -- types {{quote(file)}}

wit file="examples/value-types.wlt":
    cargo run --locked --quiet -- wit {{quote(file)}}

validate file="examples/closure.wlt":
    cargo run --locked --quiet -- check {{quote(file)}}

build file="examples/closure.wlt" output="target/program.wasm":
    cargo run --locked --quiet -- build {{quote(file)}} {{quote(output)}}

component file="tests/component-fixtures/event-loop.wlt" output="target/component.wasm":
    mkdir -p target
    cargo run --locked --quiet -- component {{quote(file)}} {{quote(output)}}

component-wat file="tests/component-fixtures/event-loop.wlt":
    cargo run --locked --quiet -- component-wat {{quote(file)}}

build-js file="tests/component-fixtures/event-loop.wlt" output="target/event-loop":
    node runtime/build.mjs {{quote(file)}} {{quote(output)}}

build-core-js file="tests/component-fixtures/event-loop.wlt" output="target/core-event-loop":
    node runtime/build-core.mjs {{quote(file)}} {{quote(output)}}

build-text output="target/text-filter":
    node runtime/build-core.mjs examples/io/text-filter.wlt {{quote(output)}}

text-filter program="target/text-filter":
    @node runtime/text-cli.mjs {{quote(program)}}

text-file input output program="target/text-filter":
    @node runtime/text-cli.mjs {{quote(program)}} --input {{quote(input)}} --output {{quote(output)}}

build-log-stats output="target/log-stats":
    node runtime/build-core.mjs examples/apps/log-stats/main.wlt {{quote(output)}}

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
    node runtime/build-core.mjs examples/apps/stock-ledger/main.wlt {{quote(output)}}

stock-ledger input="examples/apps/stock-ledger/sample.tsv" program="target/stock-ledger":
    @node runtime/text-cli.mjs {{quote(program)}} --input {{quote(input)}}

test-stock-ledger:
    cargo test --locked --test stock_ledger
    node --test runtime/stock-ledger.test.mjs

build-workflow output="target/workflow":
    node runtime/build-core.mjs examples/apps/workflow/main.wlt {{quote(output)}}

workflow input="examples/apps/workflow/sample.tsv" program="target/workflow":
    @node runtime/text-cli.mjs {{quote(program)}} --input {{quote(input)}}

# Native model tests, independent oracle, both JS backends, limits and cleanup.
test-workflow:
    cargo test --locked --test workflow
    node --expose-gc --test runtime/workflow.test.mjs

# Also send the full 65,795-line workload through both host stream adapters.
test-workflow-stress:
    WLT_WORKFLOW_STRESS=1 node --expose-gc --test runtime/workflow.test.mjs

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

test-modules:
    cargo test --locked --test modules --test static_imports --test dce
    node --test runtime/modules.test.mjs

fuzz seed="42" cases="64":
    WLT_FUZZ_SEED={{quote(seed)}} WLT_FUZZ_CASES={{quote(cases)}} cargo test --locked --test differential

fuzz-replay file:
    WLT_FUZZ_REPLAY={{quote(file)}} cargo test --locked --test differential seeded_programs

fuzz-cancel seed="42":
    WLT_FUZZ_SEED={{quote(seed)}} node --expose-gc --test runtime/fuzz.test.mjs

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
    WLT_BACKEND=core node --test runtime/runtime.test.mjs runtime/tasks.test.mjs runtime/core.test.mjs

memory-core-js:
    node --expose-gc --test runtime/core.test.mjs
    WLT_BACKEND=core node --expose-gc runtime/memory-audit.mjs

run-js output="target/event-loop" export="main" arg="1":
    node runtime/run.mjs {{quote(output)}} {{quote(export)}} {{quote(arg)}}

test-js:
    pnpm test

test-e2e:
    pnpm exec playwright test

test:
    cargo test --locked

test-language path="tests" filter="":
    cargo run --locked --quiet -- test {{quote(path)}} --filter {{quote(filter)}}

list-tests path="tests" filter="":
    cargo run --locked --quiet -- test {{quote(path)}} --filter {{quote(filter)}} --list

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

check:
    cargo fmt -- --check
    cargo clippy --locked --all-targets -- -D warnings

fmt:
    cargo fmt

fmt-wlt file="examples/apps/log-stats/main.wlt":
    cargo run --locked --quiet -- fmt {{quote(file)}}

check-wlt-format file="examples/apps/log-stats/main.wlt":
    cargo run --locked --quiet -- fmt-check {{quote(file)}}

# Check exact ownership entries and GC reachability; then sample retained memory.
memory-check:
    node --expose-gc --test runtime/memory.test.mjs runtime/retention.test.mjs
    node --expose-gc runtime/memory-audit.mjs

bind wit policy output:
    cargo run --locked --quiet -- bind {{quote(wit)}} {{quote(policy)}} {{quote(output)}}

test-bindings:
    cargo test --locked --test wit_bindings
    node --test runtime/wit-bindings.test.mjs

test-resources:
    cargo test --locked --test resources
    node --test runtime/resources.test.mjs

test-json:
    cargo test --locked --test json_values
    node --test runtime/json-value.test.mjs

test-lists:
    cargo test --locked --test lists --test wit_bindings --test differential
    node --expose-gc --test runtime/lists.test.mjs runtime/list-bindings.test.mjs

test-generics:
    cargo test --locked --test generics --test generic_bounds
    cargo run --locked --quiet -- test examples/generic-lists.wlt
    node --expose-gc --test runtime/generics.test.mjs
