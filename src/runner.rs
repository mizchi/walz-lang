//! Test discovery and execution. Test code remains ordinary typed Wasm GC code;
//! discovery, isolation, timeouts, and reporting are host responsibilities.

use crate::ir::Span;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct RunOptions {
    pub legacy_syntax: bool,
    pub filter: Option<String>,
    pub list: bool,
    /// Includes Wasmtime process startup and compilation for each test.
    pub timeout: Duration,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            legacy_syntax: false,
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

#[derive(Debug, Serialize)]
pub struct SuiteError {
    pub path: Option<PathBuf>,
    pub message: String,
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

fn discover(path: &Path, files: &mut BTreeSet<PathBuf>, report: &mut RunReport) {
    let result = (|| -> std::io::Result<()> {
        let metadata = path.metadata()?;
        if metadata.is_dir() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let kind = entry.file_type()?;
                // Do not follow directory symlinks or accidentally discover a
                // second copy through one; explicitly supplied paths are allowed.
                if kind.is_dir()
                    || (kind.is_file()
                        && entry.file_name().to_string_lossy().ends_with(".test.wlt"))
                {
                    discover(&entry.path(), files, report);
                }
            }
        } else if metadata.is_file() {
            files.insert(path.canonicalize()?);
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "expected a file or directory",
            ));
        }
        Ok(())
    })();
    if let Err(error) = result {
        report.error(Some(path), error);
    }
}

fn captured(file: &mut File) -> std::io::Result<String> {
    file.rewind()?;
    let mut bytes = Vec::new();
    file.take(64 * 1024).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_owned())
}

fn invoke(path: &Path, export: &str, timeout: Duration) -> Result<TestOutcome, String> {
    let mut stdout = tempfile::tempfile().map_err(|e| e.to_string())?;
    let mut stderr = tempfile::tempfile().map_err(|e| e.to_string())?;
    // Files avoid pipe-buffer deadlocks while the host enforces the timeout.
    let started = Instant::now();
    let mut child = Command::new("wasmtime")
        .args([
            "run",
            "-W",
            "gc=y,function-references=y,exceptions=y",
            "--invoke",
            export,
        ])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(stderr.try_clone().map_err(|e| e.to_string())?))
        .spawn()
        .map_err(|e| format!("could not start wasmtime: {e}"))?;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.to_string());
            }
        }
        if started.elapsed() >= timeout {
            child.kill().map_err(|e| e.to_string())?;
            child.wait().map_err(|e| e.to_string())?;
            return Ok(TestOutcome::TimedOut);
        }
        std::thread::sleep(Duration::from_millis(5).min(timeout.saturating_sub(started.elapsed())));
    };
    if !status.success() {
        let error = captured(&mut stderr).map_err(|e| e.to_string())?;
        return Ok(TestOutcome::Failed(format!(
            "wasmtime exited with {status}: {error}"
        )));
    }
    let output = captured(&mut stdout).map_err(|e| e.to_string())?;
    match output.parse::<i32>() {
        Ok(0) => Ok(TestOutcome::Failed("returned 0".into())),
        Ok(_) => Ok(TestOutcome::Passed),
        Err(_) => Ok(TestOutcome::Failed(format!(
            "expected an i32 result, got {output:?}"
        ))),
    }
}

/// Recursively discover `*.test.wlt` in directories; explicit files may have
/// any name. Files are deduplicated and sorted by canonical path. Empty paths
/// select `tests/` relative to the current directory. Each test runs in a fresh
/// Wasmtime process/instance. Compile errors and test failures do not stop peers.
pub fn run(paths: &[PathBuf], options: &RunOptions) -> RunReport {
    let mut report = RunReport::default();
    if options.timeout.is_zero() {
        report.error(None, "test timeout must be positive");
        return report;
    }
    let mut files = BTreeSet::new();
    if paths.is_empty() {
        discover(Path::new("tests"), &mut files, &mut report);
    }
    for path in paths {
        discover(path, &mut files, &mut report);
    }
    for path in files {
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let loaded = if options.legacy_syntax {
                crate::source::load_legacy(&path)?
            } else {
                crate::source::load(&path)?
            };
            let source = loaded.text();
            let module = crate::check(source).map_err(|e| loaded.diagnostic(e))?;
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
            let compilation =
                crate::compile_checked(source, module).map_err(|e| loaded.diagnostic(e))?;
            let directory = tempfile::tempdir()?;
            let wasm = directory.path().join("suite.wasm");
            std::fs::write(&wasm, compilation.wasm)?;
            for test in tests {
                let outcome = invoke(&wasm, &test.export, options.timeout)
                    .unwrap_or_else(TestOutcome::Failed);
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
