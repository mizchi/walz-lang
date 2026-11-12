//! Execution contract shared by test and benchmark CLI hosts.
use crate::{bindings::BindingPolicy, component::core_js::CoreJs};
use serde::Serialize;
use std::{path::PathBuf, process::Command, time::Duration};

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    #[default]
    Core,
    CoreJs,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct ExecutionOptions {
    pub target: Target,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bindings: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<PathBuf>,
}
impl ExecutionOptions {
    pub fn set_target(&mut self, value: &str) -> Result<(), String> {
        self.target = match value {
            "core" => Target::Core,
            "core-js" => Target::CoreJs,
            _ => return Err("target must be core or core-js".into()),
        };
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.host.is_some() && self.target != Target::CoreJs {
            return Err("--host requires --target core-js".into());
        }
        Ok(())
    }
    pub fn policy(&self) -> Result<BindingPolicy, Box<dyn std::error::Error>> {
        self.validate()?;
        Ok(self
            .bindings
            .as_ref()
            .map(std::fs::read_to_string)
            .transpose()?
            .map(|s| BindingPolicy::parse(&s))
            .transpose()?
            .unwrap_or_default())
    }
}

pub(crate) fn invoke(
    built: &CoreJs,
    export: &str,
    options: &ExecutionOptions,
    mode: &str,
    timeout: Duration,
    measurement: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    crate::distribution::write(directory.path(), built)?;
    for (name, source) in [
        ("cli-host.mjs", include_str!("../runtime/cli-host.mjs")),
        (
            "suite-runner.mjs",
            include_str!("../runtime/suite-runner.mjs"),
        ),
        (
            "bench-runner.mjs",
            include_str!("../runtime/bench-runner.mjs"),
        ),
        ("node-io.mjs", include_str!("../runtime/node-io.mjs")),
        ("text-io.mjs", include_str!("../runtime/text-io.mjs")),
        (
            "wasi-filesystem.mjs",
            include_str!("../runtime/wasi-filesystem.mjs"),
        ),
        (
            "node-filesystem.mjs",
            include_str!("../runtime/node-filesystem.mjs"),
        ),
    ] {
        std::fs::write(directory.path().join(name), source)?;
    }
    let entry = &built.exports[export];
    let config = serde_json::json!({"mode":mode,"entry":entry.js_name,"arity":entry.arity,"timeout":timeout.as_millis(),"measurement":measurement,"host":options.host.as_ref().map(std::fs::canonicalize).transpose()?});
    let file = directory.path().join("suite.json");
    std::fs::write(&file, serde_json::to_vec(&config)?)?;
    // Node requests cooperative cancellation at the deadline, and gets a
    // bounded grace period to unwind guest stacks and dispose host resources.
    let output = crate::harness::invoke(
        Command::new("node")
            .arg(directory.path().join("suite-runner.mjs"))
            .arg(file),
        timeout.saturating_add(Duration::from_secs(5)),
        1024 * 1024,
    )?;
    let Some(output) = output else {
        return Err("Node did not finish cancellation within the 5s cleanup grace period".into());
    };
    if !output.status.success() {
        return Err(format!(
            "Node exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    serde_json::from_slice(&output.stdout).map_err(|e| {
        format!(
            "invalid suite output: {e}: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into()
    })
}
