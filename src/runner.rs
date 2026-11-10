//! Test discovery and execution. Test code remains ordinary typed Wasm GC code;
//! discovery, isolation, timeouts, and reporting are host responsibilities.

pub use crate::harness::SuiteError;
use crate::ir::Span;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct RunOptions {
    /// Internal hook for frozen evaluation inputs.
    #[doc(hidden)]
    pub legacy_syntax: bool,
    pub execution: crate::execution::ExecutionOptions,
    pub filter: Option<String>,
    pub list: bool,
    /// Includes Wasmtime process startup and compilation for each test.
    pub timeout: Duration,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            legacy_syntax: false,
            execution: Default::default(),
            filter: None,
            list: false,
            timeout: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", content = "message", rename_all = "snake_case")]
pub enum TestOutcome {
    Passed,
    Failed(String),
    TimedOut,
    Listed,
}

#[derive(Debug, Serialize)]
pub struct TestResult {
    pub path: PathBuf,
    pub name: String,
    pub span: Span,
    pub outcome: TestOutcome,
}

#[derive(Debug, Default, Serialize)]
pub struct RunReport {
    pub tests: Vec<TestResult>,
    pub errors: Vec<SuiteError>,
    pub filtered_out: usize,
}

impl RunReport {
    pub fn passed(&self) -> usize {
        self.tests
            .iter()
            .filter(|test| matches!(test.outcome, TestOutcome::Passed))
            .count()
    }

    pub fn failed(&self) -> usize {
        self.tests
            .iter()
            .filter(|test| matches!(test.outcome, TestOutcome::Failed(_) | TestOutcome::TimedOut))
            .count()
    }

    pub fn success(&self) -> bool {
        !self.tests.is_empty() && self.errors.is_empty() && self.failed() == 0
    }

    fn error(&mut self, path: Option<&Path>, message: impl std::fmt::Display) {
        self.errors.push(SuiteError {
            path: path.map(Path::to_owned),
            message: message.to_string(),
        });
    }
}

fn invoke(path: &Path, export: &str, timeout: Duration) -> Result<TestOutcome, String> {
    let output = crate::harness::invoke(
        Command::new("wasmtime")
            .args([
                "run",
                "-W",
                "gc=y,function-references=y,exceptions=y",
                "--invoke",
                export,
            ])
            .arg(path),
        timeout,
        64 * 1024,
    )
    .map_err(|e| format!("could not start wasmtime: {e}"))?;
    let Some(output) = output else {
        return Ok(TestOutcome::TimedOut);
    };
    let status = output.status;
    if !status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Ok(TestOutcome::Failed(format!(
            "wasmtime exited with {status}: {error}"
        )));
    }
    let output = String::from_utf8_lossy(&output.stdout);
    let output = output.trim();
    match output.parse::<i32>() {
        Ok(0) => Ok(TestOutcome::Failed("returned 0".into())),
        Ok(_) => Ok(TestOutcome::Passed),
        Err(_) => Ok(TestOutcome::Failed(format!(
            "expected an i32 result, got {output:?}"
        ))),
    }
}

/// Recursively discover `*.test.walz` in directories; explicit files may have
/// any name. Files are deduplicated and sorted by canonical path. Empty paths
/// select `tests/` relative to the current directory. Each test runs in a fresh
/// Wasmtime process/instance. Compile errors and test failures do not stop peers.
pub fn run(paths: &[PathBuf], options: &RunOptions) -> RunReport {
    let mut report = RunReport::default();
    if options.timeout.is_zero() {
        report.error(None, "test timeout must be positive");
        return report;
    }
    let policy = match options.execution.policy() {
        Ok(policy) => policy,
        Err(error) => {
            report.error(None, error);
            return report;
        }
    };
    let discovery = crate::harness::discover(paths, "tests", ".test.walz");
    report.errors.extend(discovery.errors);
    for path in discovery.files {
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let loaded = if options.legacy_syntax {
                crate::source::load_legacy(&path)?
            } else {
                crate::source::load(&path)?
            };
            let source = loaded.text();
            let module =
                crate::check_with_policy(source, &policy).map_err(|e| loaded.diagnostic(e))?;
            let tests = module
                .tests
                .iter()
                .filter(|test| {
                    options
                        .filter
                        .as_ref()
                        .is_none_or(|filter| test.name.contains(filter))
                })
                .cloned()
                .collect::<Vec<_>>();
            report.filtered_out += module.tests.len() - tests.len();
            if tests.is_empty() {
                return Ok(());
            }
            if options.list {
                report
                    .tests
                    .extend(tests.into_iter().map(|test| TestResult {
                        path: path.clone(),
                        name: test.name,
                        span: test.span,
                        outcome: TestOutcome::Listed,
                    }));
                return Ok(());
            }
            let directory = tempfile::tempdir()?;
            let wasm = directory.path().join("suite.wasm");
            if options.execution.target == crate::execution::Target::Core {
                let compilation = crate::compile_checked(source, module.clone())
                    .map_err(|e| loaded.diagnostic(e))?;
                std::fs::write(&wasm, compilation.wasm)?;
            }
            for test in tests {
                let execute = || -> Result<TestOutcome, Box<dyn std::error::Error>> {
                    if options.execution.target == crate::execution::Target::Core {
                        return invoke(&wasm, &test.export, options.timeout).map_err(Into::into);
                    }
                    let built = crate::component::core_js::compile_case(
                        source,
                        &module,
                        test.function,
                        "walz-case",
                    )
                    .map_err(|e| loaded.diagnostic(e))?;
                    let report = crate::execution::invoke(
                        &built,
                        "walz-case",
                        &options.execution,
                        "test",
                        options.timeout,
                        serde_json::Value::Null,
                    )?;
                    Ok(match report["status"].as_str() {
                        Some("passed") => TestOutcome::Passed,
                        Some("timed_out") => TestOutcome::TimedOut,
                        _ => TestOutcome::Failed(
                            report["message"]
                                .as_str()
                                .unwrap_or("invalid test outcome")
                                .into(),
                        ),
                    })
                };
                let outcome = execute().unwrap_or_else(|e| TestOutcome::Failed(e.to_string()));
                report.tests.push(TestResult {
                    path: path.clone(),
                    name: test.name,
                    span: test.span,
                    outcome,
                });
            }
            Ok(())
        })();
        if let Err(error) = result {
            report.error(Some(&path), error);
        }
    }
    if report.tests.is_empty() && report.errors.is_empty() {
        report.error(None, "no tests matched");
    }
    report
}
