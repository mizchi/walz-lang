//! Fail-closed execution of the typed core through an independent ownership
//! contract kernel. Parsing, extraction, effects, lowering and Wasm remain TCB.

use crate::ir::FunctionKind;
use crate::{Compilation, Error};
use std::io::Write;
use std::process::Command;

pub const LEAN_VERSION: &str = "+v4.34.1";
pub const KERNEL_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/proof/Ownership.lean");

/// Check a single IR, verify its ownership contracts, then emit that same IR.
/// No native-WAT fallback, unknown imports, or owned values at raw exports.
pub fn compile(source: &str) -> Result<Compilation, Error> {
    let module = crate::check(source)
        .map_err(|error| error.note("verified mode requires the typed core"))?;
    verify(source, &module)?;
    crate::emit_checked(module)
}

pub(crate) fn verify(source: &str, module: &crate::ir::Module) -> Result<(), Error> {
    verify_policy(source, module, false)
}

pub(crate) fn verify_component(source: &str, module: &crate::ir::Module) -> Result<(), Error> {
    verify_policy(source, module, true)
}

fn verify_policy(source: &str, module: &crate::ir::Module, component: bool) -> Result<(), Error> {
    let kinds = crate::ownership::Kinds::new(module);
    for (index, function) in module.functions.iter().enumerate() {
        if matches!(function.kind, FunctionKind::Import { .. })
            && !(component && crate::bindings::lookup(function).is_some())
        {
            return Err(Error::message(
                "verified mode does not accept imports without trusted bindings".into(),
            ));
        }
        if !function.exports.is_empty() {
            if !component
                && (kinds.is_owned(&function.result)
                    || function
                        .params
                        .iter()
                        .any(|id| kinds.is_owned(&function.locals[id.0].ty)))
            {
                return Err(Error::message(
                    "verified mode does not expose owned exports through the raw Wasm ABI".into(),
                ));
            }
            if !module.tests.iter().any(|test| test.function.0 == index)
                && function.effect_contract.is_none()
            {
                return Err(Error::message(format!(
                    "verified export `{}` requires an effect contract",
                    function.name
                )));
            }
        }
    }
    let contracts = crate::ownership::validate(source, module)?;
    let mut file = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .map_err(|error| Error::message(error.to_string()))?;
    serde_json::to_writer(&mut file, &contracts)
        .map_err(|error| Error::message(error.to_string()))?;
    file.flush()
        .map_err(|error| Error::message(error.to_string()))?;
    let mut proof = tempfile::Builder::new()
        .suffix(".lean")
        .tempfile()
        .map_err(|error| Error::message(error.to_string()))?;
    proof
        .write_all(crate::proof::emit(module)?.as_bytes())
        .map_err(|error| Error::message(error.to_string()))?;
    proof
        .flush()
        .map_err(|error| Error::message(error.to_string()))?;
    let output = Command::new("lean")
        .arg(LEAN_VERSION)
        .arg("--run")
        .arg(proof.path())
        .arg(file.path())
        .output()
        .map_err(|error| Error::message(format!("could not start ownership kernel: {error}")))?;
    if !output.status.success() {
        let laws = module
            .laws
            .iter()
            .enumerate()
            .map(|(index, law)| format!("law{index}: {:?}", law.name))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(Error::message(format!(
            "contract kernel rejected obligations ({laws}):\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}
