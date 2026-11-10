//! Filesystem and process mechanics shared by the test and benchmark hosts.
use serde::Serialize;
use std::collections::BTreeSet;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Select an application only after validating every declaration. Harness
/// entry points are capabilities of test/bench runners, not public ABI roots.
pub(crate) fn select_application(module: &mut crate::ir::Module) {
    for (function, export) in module
        .benches
        .iter()
        .map(|b| (b.function, &b.export))
        .chain(module.tests.iter().map(|t| (t.function, &t.export)))
    {
        module.functions[function.0]
            .exports
            .retain(|name| name != export);
    }
    module.tests.clear();
    module.benches.clear();
}

#[derive(Debug, Serialize)]
pub struct SuiteError {
    pub path: Option<PathBuf>,
    pub message: String,
}

#[derive(Default)]
pub(crate) struct Discovery {
    pub files: BTreeSet<PathBuf>,
    pub errors: Vec<SuiteError>,
}

pub(crate) fn discover(paths: &[PathBuf], default: &str, suffix: &str) -> Discovery {
    fn visit(path: &Path, suffix: &str, out: &mut Discovery) {
        let result = (|| -> std::io::Result<()> {
            let metadata = path.metadata()?;
            if metadata.is_dir() {
                for entry in std::fs::read_dir(path)? {
                    let entry = entry?;
                    let kind = entry.file_type()?;
                    // Only explicitly supplied paths may follow symlinks.
                    // Build artifacts and dependencies are opt-in search roots.
                    let generated = matches!(
                        entry.file_name().to_str(),
                        Some("target" | "node_modules" | ".git")
                    );
                    if (kind.is_dir() && !generated)
                        || (kind.is_file()
                            && (entry.file_name().to_string_lossy().ends_with(suffix)
                                || suffix.strip_suffix(".walz").is_some_and(|base| {
                                    entry
                                        .file_name()
                                        .to_string_lossy()
                                        .ends_with(&format!("{base}.watz"))
                                })))
                    {
                        visit(&entry.path(), suffix, out);
                    }
                }
            } else if metadata.is_file() {
                out.files.insert(path.canonicalize()?);
            } else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "expected a file or directory",
                ));
            }
            Ok(())
        })();
        if let Err(error) = result {
            out.errors.push(SuiteError {
                path: Some(path.to_owned()),
                message: error.to_string(),
            });
        }
    }
    let mut out = Discovery::default();
    if paths.is_empty() {
        visit(Path::new(default), suffix, &mut out);
    }
    for path in paths {
        visit(path, suffix, &mut out);
    }
    out
}

/// Capture to files to avoid pipe deadlocks. None means the child timed out;
/// all termination paths reap it before returning. The limit is per stream.
pub(crate) fn invoke(
    command: &mut Command,
    timeout: Duration,
    limit: u64,
) -> std::io::Result<Option<Output>> {
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    let started = Instant::now();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.try_clone()?))
        .stderr(Stdio::from(stderr.try_clone()?))
        .spawn()?;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        }
        if started.elapsed() >= timeout {
            let killed = child.kill();
            let waited = child.wait();
            killed?;
            waited?;
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(5).min(timeout.saturating_sub(started.elapsed())));
    };
    fn capture(file: &mut std::fs::File, limit: u64) -> std::io::Result<Vec<u8>> {
        file.rewind()?;
        let mut bytes = Vec::new();
        file.take(limit).read_to_end(&mut bytes)?;
        Ok(bytes)
    }
    Ok(Some(Output {
        status,
        stdout: capture(&mut stdout, limit)?,
        stderr: capture(&mut stderr, limit)?,
    }))
}
