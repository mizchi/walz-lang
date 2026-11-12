# FlatBuffers release TODO

This is the follow-up plan for turning the codec and synchronous JS ABI
experiments into a public release. Unchecked items are future work; the proposed
CLI below is not implemented. Keep these changes separate from the Walz language
specification.

## Release direction

- Keep UTF-8 as the shared string semantics: byte lengths/offsets, strict invalid
  UTF-8 rejection, BOM retention and the existing JS surrogate normalization.
- Keep the full runtime as the default for `--target core-js`, including its
  queued `start()`, cancellation and resource protocols.
- Offer the minimal UTF-8 runtime as an explicit preview for synchronous
  shared-value libraries. Reject unsupported features with actionable errors.
- Keep native JS strings experimental until application benchmarks, memory use
  and engine compatibility justify promotion.
- Keep Component as the WIT/WASI and cross-language interoperability path. The
  private minimal JS ABI is not a replacement for the Canonical ABI.
- Preserve input validation, immutable bytes snapshots, type/effect/ownership
  checks and the existing proof gates in every supported configuration.

Do not automatically switch between full and minimal runtimes when source
changes would alter the availability of public APIs such as `start()`.

## Evidence already available

- [x] Packed i8 builder storage, bulk copies, synchronous-call allocation
  reduction, checked queries, fused transforms and batches:
  [implementation and measurements](../../benchmarks/flatbuffers/20261111-three-improvements-v1/README.md).
- [x] Direct GC boundaries with UTF-8 and native JS string representations:
  [design and restrictions](../../docs/js-native-experiment.md),
  [final measurements](../../benchmarks/flatbuffers/20261111-js-native-v1/README.md).
- [x] Strict UTF-8/BOM/snapshot checks, argument-error and guest-trap GC retention
  tests, and direct/bundled browser checks for the experimental paths.

The measured Message codec is 14,742 B gzip with the full Core runtime,
8,285 B with minimal UTF-8, and 5,969 B with minimal UTF-8 plus Binaryen 133 O3.
Native JS strings measure 8,428 B / 6,086 B respectively; official JS plus the
handwritten schema adapter measures 4,003 B. These are workload-specific
baselines, not universal size budgets. The JS reader does not perform equivalent
structural verification, and minimal output omits full-runtime capabilities.

## 1. Share boundary contracts and tests

- [ ] Extract common boundary shape/validation rules from
  [the portable codecs](../../src/component/core_js/values.rs) and
  [the experimental codecs](../../src/component/js_native/values.rs), leaving
  transport and physical representation in their respective backends.
- [ ] Define a single capability matrix for full Core, minimal UTF-8 and
  Component. Track the preview's supported subset explicitly.
- [ ] Close or explicitly diagnose the minimal backend's missing nominal scalar,
  enum, flags and variant interfaces. Require complete supported-shape coverage
  before declaring the minimal runtime stable.
- [ ] Run common cases for valid values, wrong JS types, integer limits,
  Unicode/BOM/surrogates, nested containers, malformed buffers and snapshots.
  Cover reentry, pre-entry validation errors, fatal guest/intrinsic failures and
  attempts to swallow intrinsic failures with `catch_all`.
- [ ] Keep unsupported suspension, owners, host capabilities and used source
  memories as compile errors. Explain the restriction and how to select the
  full runtime; never silently discard a capability.

Done when shared contracts drive the backends and their applicable test cases,
with documented differences in capabilities rather than divergent value rules.

## 2. Integrate the preview into the development workflow

Proposed interface:

```sh
walz build app.walz --target core-js
walz build app.walz --target core-js --runtime minimal
walz build app.walz --target component
```

- [ ] Add explicit runtime selection to CLI/project configuration, keeping the
  current full-runtime default. Validate incompatible target/runtime choices.
- [ ] Integrate selection with Vite and include it in build/cache identities.
- [ ] Generate type declarations and checked ESM metadata for the selected API.
  Minimal declarations must not advertise `start()` or cancellation facilities.
- [ ] Version the ABI/capability manifest and document loader requirements,
  transfer limits, scratch-memory high-water retention and failure behavior.
- [ ] Add CLI, installed-package and Vite build/load tests, then update English
  documentation and both README/cheat-sheet languages where applicable.
- [ ] Keep native JS strings out of the stable CLI surface for now; retain the
  existing experimental tasks for evaluating them.

Done when a user can explicitly build and import the preview through the normal
workflow, with accurate types, reproducible caching and useful diagnostics.

## 3. Establish release checks and measurements

- [ ] Put the shared ABI, malformed-input, cleanup and GC retention checks in CI.
  Exercise full Core plus real JSPI/cancellation, minimal UTF-8 and Component.
- [ ] Record a tested Node/browser compatibility matrix, expanding beyond the
  current Node/Chromium evidence before making broader support claims.
- [ ] Establish reviewed size/throughput regression limits for named fixtures.
  Measure deployed JS plus Wasm, compressed sizes, cold initialization and
  copy-inclusive calls separately from resident GC processing.
- [ ] Add peak-memory and steady-state measurements that distinguish live GC
  data, reusable linear capacity and external resources.
- [ ] Apply a pinned, validated wasm-opt configuration to published artifacts.
  Start from the measured Binaryen 133 O3 settings; compare other settings before
  adopting them. Preserve traps and avoid unverified closed-world assumptions.
  Normal local builds must not require an implicit Binaryen installation.
- [ ] Add an equivalently checked JS comparison, or clearly label verification
  differences. Limit public performance claims to the measured workloads.

Done when the release pipeline reproduces the compatibility, correctness and
measurement evidence for its actual deployed artifacts.

## Later: evaluate native JS strings

- [ ] Test realistic string-heavy applications beyond echo and byte length.
- [ ] Measure the cost of retaining both a JS string and its UTF-8 cache.
- [ ] Investigate lowering applicable core string operations to JS string
  builtins while preserving UTF-8 offsets and source semantics. The current
  experiment uses `cast` and imported constants, not native concat/substring.
- [ ] Define feature detection, loader behavior and an explicit unsupported-host
  policy. Do not assume JS String Builtins also provide UTF-8 conversion builtins.
- [ ] Revisit promotion only after those results; no new language syntax or
  mandatory experimental Text Encoding Builtins dependency is needed for the
  proposed release.

## Resume here

Start with section 1, then sections 2 and 3. Existing entry points:

```sh
just test-flatbuffers
just test-js-native
just test-js-native-e2e
just js-native-capture target/fb-release-capture
just bench-js-native target/fb-release-capture target/fb-release-run
just size-js-native target/fb-release-capture target/fb-release-size /path/to/binaryen-133/bin/wasm-opt
```

Capture/run/size outputs must use fresh directories. Preserve existing inputs,
artifacts and measurements; version the suite when tasks or reference programs
change. See the linked records before interpreting historical comparisons.
