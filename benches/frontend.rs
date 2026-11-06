use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::process::{Command, Stdio};
use std::{hint::black_box, time::Duration};

fn config() -> Criterion {
    let directory = std::env::var_os("CRITERION_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| {
                    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target")
                })
                .join("criterion")
        })
        .join("frontend-v4");
    Criterion::default()
        .output_directory(&directory)
        .sample_size(30)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
}

const CASES: &[(&str, &str)] = &[
    ("native", include_str!("fixtures/frontend-v4/native.wlt")),
    ("let", include_str!("fixtures/frontend-v4/let.wlt")),
    (
        "std-types",
        include_str!("fixtures/frontend-v4/std-types.wlt"),
    ),
    (
        "std-string",
        include_str!("fixtures/frontend-v4/std-string.wlt"),
    ),
    ("json", include_str!("fixtures/frontend-v4/json.wlt")),
    (
        "stock-ledger",
        include_str!("fixtures/frontend-v4/stock-ledger.wlt"),
    ),
    (
        "workflow",
        include_str!("fixtures/frontend-v4/workflow.wlt"),
    ),
];

fn frontend(c: &mut Criterion) {
    for (operation, compile) in [("check", false), ("compile", true)] {
        let mut group = c.benchmark_group(operation);
        for &(name, source) in CASES {
            group.throughput(Throughput::Bytes(source.len() as u64));
            group.bench_with_input(BenchmarkId::from_parameter(name), source, |b, source| {
                if compile {
                    b.iter(|| black_box(wlt::compile(black_box(source)).unwrap()));
                } else {
                    b.iter(|| black_box(wlt::check(black_box(source)).unwrap()));
                }
            });
        }
        group.finish();
    }
    c.bench_function("check-error/std-type-mismatch", |b| {
        let source = include_str!("fixtures/frontend-v4/type-error.wlt");
        b.iter(|| black_box(wlt::check(black_box(source)).unwrap_err()));
    });
    let mut cold = c.benchmark_group("cold-check");
    for name in ["std-types", "json"] {
        let path = format!(
            "{}/benches/fixtures/frontend-v4/{name}.wlt",
            env!("CARGO_MANIFEST_DIR")
        );
        cold.bench_function(name, |b| {
            b.iter(|| {
                let status = Command::new(env!("CARGO_BIN_EXE_wlt"))
                    .args(["check", "--legacy-syntax", &path])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .unwrap();
                assert!(status.success());
            })
        });
    }
    cold.finish();
}

criterion_group! {
    name = benches;
    config = config();
    targets = frontend
}
criterion_main!(benches);
