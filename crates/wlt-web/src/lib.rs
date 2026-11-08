//! The browser bridge uses the same compiler and structured diagnostics as the CLI.

use serde::Serialize;

#[derive(Serialize)]
struct Report<T> {
    schema: &'static str,
    #[serde(flatten)]
    outcome: T,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Outcome {
    Success {
        wat: String,
        wasm: Vec<u8>,
        analysis: wlt::Analysis,
        #[serde(skip_serializing_if = "Option::is_none")]
        execution: Option<wlt::component::core_js::CoreJs>,
    },
    Error {
        diagnostic: wlt::diagnostic::Diagnostic,
    },
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum ExecutionOutcome {
    Success {
        program: wlt::component::core_js::CoreJs,
    },
    Error {
        diagnostic: wlt::diagnostic::Diagnostic,
    },
}

/// Prepare the existing Core/JSPI ABI in memory, without native tools or I/O.
/// This applies Rust checks and boundary policy; independent Lean verification
/// remains a native CLI feature, and laws are rejected by this entry point.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn prepare_execution(source: &str, optimize: bool) -> String {
    let outcome = match wlt::component::core_js::compile_for_browser(
        source,
        wlt::CompileOptions { optimize },
    ) {
        Ok(program) => ExecutionOutcome::Success { program },
        Err(error) => ExecutionOutcome::Error {
            diagnostic: error.diagnostic().clone(),
        },
    };
    serde_json::to_string(&Report {
        schema: "wlt-playground-execution-v1",
        outcome,
    })
    .expect("execution report is serializable")
}

/// JSON envelope `wlt-playground-v1`. Compilation failures carry the original
/// structured diagnostic and never contain stale executable bytes.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn compile_source(source: &str, optimize: bool) -> String {
    let outcome = match wlt::compile_with_options(source, wlt::CompileOptions { optimize }) {
        Ok(compilation) => Outcome::Success {
            wat: compilation.wat,
            wasm: compilation.wasm,
            analysis: compilation.analysis,
            execution: None,
        },
        Err(error) if error.diagnostic().code == "E_TASK_BACKEND" => {
            match wlt::component::core_js::compile_for_browser(
                source,
                wlt::CompileOptions { optimize },
            )
            .and_then(|program| {
                wlt::check(source).map(|module| (program, wlt::analysis::analyze(&module)))
            }) {
                Ok((mut program, analysis)) => Outcome::Success {
                    wat: std::mem::take(&mut program.wat),
                    wasm: program.wasm.clone(),
                    analysis,
                    execution: Some(program),
                },
                Err(error) => Outcome::Error {
                    diagnostic: error.diagnostic().clone(),
                },
            }
        }
        Err(error) => Outcome::Error {
            diagnostic: error.diagnostic().clone(),
        },
    };
    serde_json::to_string(&Report {
        schema: "wlt-playground-v1",
        outcome,
    })
    .expect("compiler report is serializable")
}
