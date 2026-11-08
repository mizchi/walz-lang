//! Benchmark declarations stay ordinary typed Wasm functions. This host layer
//! discovers suites, isolates cases, and measures resident Wasm batches in Node.

use crate::{Error, ir::*};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Clone, Debug, Serialize)]
pub struct BenchOptions {
    #[serde(flatten)]
    pub execution: crate::execution::ExecutionOptions,
    pub filter: Option<String>,
    pub list: bool,
    pub samples: u32,
    pub iterations: Option<u32>,
    pub warmup_ms: u64,
    pub sample_ms: u64,
    /// Includes Node startup, Wasm compilation, warmup, and measurement per case.
    pub timeout_ms: u64,
}

impl Default for BenchOptions {
    fn default() -> Self {
        Self {
            execution: Default::default(),
            filter: None,
            list: false,
            samples: 10,
            iterations: None,
            warmup_ms: 100,
            sample_ms: 10,
            timeout_ms: 30_000,
        }
    }
}

impl BenchOptions {
    pub fn validate(&self) -> Result<(), String> {
        self.execution.validate()?;
        if !(1..=10_000).contains(&self.samples) {
            return Err("--samples must be between 1 and 10000".into());
        }
        if self
            .iterations
            .is_some_and(|n| !(1..=i32::MAX as u32).contains(&n))
        {
            return Err("--iterations must be between 1 and 2147483647".into());
        }
        if self.sample_ms == 0 || self.timeout_ms == 0 {
            return Err("--sample-ms and --timeout-ms must be positive".into());
        }
        // JavaScript represents these millisecond values as exact integers.
        if [self.warmup_ms, self.sample_ms, self.timeout_ms]
            .iter()
            .any(|&n| n > (1u64 << 53) - 1)
        {
            return Err("millisecond options must be at most 9007199254740991".into());
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub iterations: u32,
    /// Raw elapsed nanoseconds per operation, in measurement order.
    pub samples_ns: Vec<f64>,
    pub min_ns: f64,
    pub max_ns: f64,
    pub mean_ns: f64,
    pub median_ns: f64,
    /// Sample standard deviation (n - 1); zero for a single sample.
    pub std_dev_ns: f64,
    pub runtime: String,
}

#[derive(Deserialize)]
struct Measurement {
    iterations: u32,
    samples_ns: Vec<f64>,
    runtime: String,
}

impl Summary {
    fn from_measurement(value: Measurement, options: &BenchOptions) -> Result<Self, String> {
        let samples = &value.samples_ns;
        if value.iterations == 0
            || value.iterations > i32::MAX as u32
            || options.iterations.is_some_and(|n| n != value.iterations)
            || samples.len() != options.samples as usize
            || samples.iter().any(|n| !n.is_finite() || *n < 0.0)
        {
            return Err("invalid benchmark measurements from Node".into());
        }
        let mut sorted = samples.clone();
        sorted.sort_by(f64::total_cmp);
        let count = samples.len();
        let mean = samples.iter().sum::<f64>() / count as f64;
        let std_dev = if count == 1 {
            0.0
        } else {
            (samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (count - 1) as f64).sqrt()
        };
        Ok(Self {
            iterations: value.iterations,
            min_ns: sorted[0],
            max_ns: sorted[count - 1],
            mean_ns: mean,
            median_ns: (sorted[(count - 1) / 2] + sorted[count / 2]) / 2.0,
            std_dev_ns: std_dev,
            samples_ns: value.samples_ns,
            runtime: value.runtime,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BenchOutcome {
    Measured { summary: Summary },
    Failed { message: String },
    TimedOut,
    Listed,
}

#[derive(Debug, Serialize)]
pub struct BenchResult {
    pub path: PathBuf,
    pub name: String,
    pub span: Span,
    pub outcome: BenchOutcome,
}

#[derive(Debug, Serialize)]
pub struct BenchReport {
    pub schema: &'static str,
    pub options: BenchOptions,
    pub benchmarks: Vec<BenchResult>,
    pub errors: Vec<crate::runner::SuiteError>,
    pub filtered_out: usize,
}

impl BenchReport {
    pub fn measured(&self) -> usize {
        self.benchmarks
            .iter()
            .filter(|b| matches!(b.outcome, BenchOutcome::Measured { .. }))
            .count()
    }
    pub fn failed(&self) -> usize {
        self.benchmarks
            .iter()
            .filter(|b| {
                matches!(
                    b.outcome,
                    BenchOutcome::Failed { .. } | BenchOutcome::TimedOut
                )
            })
            .count()
    }
    pub fn success(&self) -> bool {
        !self.benchmarks.is_empty() && self.errors.is_empty() && self.failed() == 0
    }
    fn error(&mut self, path: Option<&Path>, message: impl std::fmt::Display) {
        self.errors.push(crate::runner::SuiteError {
            path: path.map(Path::to_owned),
            message: message.to_string(),
        });
    }
}

pub(crate) fn validate(source: &str, module: &Module) -> Result<(), Error> {
    if module.benches.is_empty() {
        return Ok(());
    }
    let kinds = crate::ownership::Kinds::new(module);
    for bench in &module.benches {
        let function = &module.functions[bench.function.0];
        if kinds.is_owned(&function.result) || matches!(function.result, Type::Borrow(_)) {
            return Err(Error::at(
                source,
                bench.span,
                "benchmark results cannot be owned or borrowed; consume resources inside the body and return shared data or unit",
            ));
        }
    }
    Ok(())
}

/// Compile a selected case into a batch entry point and an observable result
/// global. Other exports/tests/benchmarks are not roots of this executable.
pub fn compile(source: &str, module: &Module, index: usize) -> Result<Vec<u8>, Error> {
    let bench = module
        .benches
        .get(index)
        .ok_or_else(|| Error::message("benchmark index out of range".into()))?;
    let mut selected = module.clone();
    selected.tests.clear();
    selected.benches = vec![bench.clone()];
    for function in &mut selected.functions {
        function.exports.retain(|name| name == &bench.export);
    }
    // Retain normal verification, including laws, before producing the harness.
    let checked = crate::compile_checked(source, selected)?;
    let module = checked.ir.unwrap();
    let lowered =
        crate::newtypes::lower(&crate::link::project(&crate::optimize::simplify(&module)));
    if lowered
        .functions
        .iter()
        .any(|f| matches!(f.kind, FunctionKind::Import { .. }))
        || lowered.memories.iter().any(|m| m.import.is_some())
    {
        return Err(Error::at(
            source,
            bench.span,
            "benchmark runner does not provide host imports; benchmark a closed synchronous guest function",
        ));
    }
    if crate::analysis::analyze(&module).functions[bench.function.0]
        .effects
        .may_suspend
    {
        return Err(Error::at(
            source,
            bench.span,
            "benchmark bodies must be synchronous; suspension is not supported by the benchmark runner",
        ));
    }
    let wat = crate::emit::benchmark(&lowered, &bench.export);
    let wasm = wat::parse_str(&wat).map_err(|e| Error::message(format!("benchmark WAT: {e}")))?;
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .map_err(|e| Error::message(format!("benchmark Wasm: {e}")))?;
    Ok(wasm)
}

fn invoke(wasm: &Path, driver: &Path, options: &BenchOptions) -> Result<BenchOutcome, String> {
    let output = crate::harness::invoke(
        Command::new("node")
            .arg(driver)
            .arg(wasm)
            .arg(serde_json::to_string(options).map_err(|e| e.to_string())?),
        Duration::from_millis(options.timeout_ms),
        1024 * 1024,
    )
    .map_err(|e| format!("could not execute Node.js 24+: {e}"))?;
    let Some(output) = output else {
        return Ok(BenchOutcome::TimedOut);
    };
    let status = output.status;
    if !status.success() {
        return Err(format!(
            "Node exited with {status}: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let measured = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("invalid Node benchmark output: {e}"))?;
    Ok(BenchOutcome::Measured {
        summary: Summary::from_measurement(measured, options)?,
    })
}

/// Discover `*_bench.wlt` recursively (default: current directory). Explicit
/// files may have any name. Every case has its own process and Wasm instance;
/// warmup, calibration, and samples share that instance. Errors do not stop peers.
pub fn run(paths: &[PathBuf], options: &BenchOptions) -> BenchReport {
    let mut report = BenchReport {
        schema: "wlt-bench-v1",
        options: options.clone(),
        benchmarks: vec![],
        errors: vec![],
        filtered_out: 0,
    };
    if let Err(error) = options.validate() {
        report.error(None, error);
        return report;
    }
    let policy = match options.execution.policy() {
        Ok(policy) => policy,
        Err(error) => {
            report.error(None, error);
            return report;
        }
    };
    let discovery = crate::harness::discover(paths, ".", "_bench.wlt");
    report.errors.extend(discovery.errors);
    for path in discovery.files {
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let loaded = crate::source::load(&path)?;
            let source = loaded.text();
            let module =
                crate::check_with_policy(source, &policy).map_err(|e| loaded.diagnostic(e))?;
            for (index, bench) in module.benches.iter().enumerate() {
                if options
                    .filter
                    .as_ref()
                    .is_some_and(|filter| !bench.name.contains(filter))
                {
                    report.filtered_out += 1;
                    continue;
                }
                let outcome = if options.list {
                    BenchOutcome::Listed
                } else {
                    let execute = (|| -> Result<BenchOutcome, Box<dyn std::error::Error>> {
                        if options.execution.target == crate::execution::Target::CoreJs {
                            let built = crate::component::core_js::compile_case(
                                source,
                                &module,
                                bench.function,
                                "wlt-case",
                            )
                            .map_err(|e| loaded.diagnostic(e))?;
                            let report = crate::execution::invoke(
                                &built,
                                "wlt-case",
                                &options.execution,
                                "bench",
                                Duration::from_millis(options.timeout_ms),
                                serde_json::to_value(options)?,
                            )?;
                            return Ok(match report["status"].as_str() {
                                Some("timed_out") => BenchOutcome::TimedOut,
                                Some("measured") => BenchOutcome::Measured {
                                    summary: Summary::from_measurement(
                                        serde_json::from_value(report["measurement"].clone())?,
                                        options,
                                    )?,
                                },
                                _ => BenchOutcome::Failed {
                                    message: report["message"]
                                        .as_str()
                                        .unwrap_or("invalid benchmark outcome")
                                        .into(),
                                },
                            });
                        }
                        let wasm =
                            compile(source, &module, index).map_err(|e| loaded.diagnostic(e))?;
                        let directory = tempfile::tempdir()?;
                        let file = directory.path().join("bench.wasm");
                        let driver = directory.path().join("bench.mjs");
                        std::fs::write(&file, wasm)?;
                        std::fs::write(&driver, include_str!("../runtime/bench-runner.mjs"))?;
                        Ok(invoke(&file, &driver, options).map_err(std::io::Error::other)?)
                    })();
                    execute.unwrap_or_else(|e| BenchOutcome::Failed {
                        message: e.to_string(),
                    })
                };
                report.benchmarks.push(BenchResult {
                    path: path.clone(),
                    name: bench.name.clone(),
                    span: bench.span,
                    outcome,
                });
            }
            Ok(())
        })();
        if let Err(error) = result {
            report.error(Some(&path), error);
        }
    }
    if report.benchmarks.is_empty() && report.errors.is_empty() {
        report.error(None, "no benchmarks matched");
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn statistics_preserve_raw_samples_and_use_sample_deviation() {
        let summary = Summary::from_measurement(
            Measurement {
                iterations: 4,
                samples_ns: vec![4.0, 1.0, 3.0, 2.0],
                runtime: "test".into(),
            },
            &BenchOptions {
                samples: 4,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(summary.samples_ns, [4.0, 1.0, 3.0, 2.0]);
        assert_eq!(
            (
                summary.min_ns,
                summary.max_ns,
                summary.mean_ns,
                summary.median_ns
            ),
            (1.0, 4.0, 2.5, 2.5)
        );
        assert!((summary.std_dev_ns - (5.0_f64 / 3.0).sqrt()).abs() < 1e-12);
        for samples in [vec![], vec![f64::NAN], vec![-1.0]] {
            assert!(
                Summary::from_measurement(
                    Measurement {
                        iterations: 1,
                        samples_ns: samples,
                        runtime: "test".into()
                    },
                    &BenchOptions {
                        samples: 1,
                        ..Default::default()
                    }
                )
                .is_err()
            );
        }
    }
}
