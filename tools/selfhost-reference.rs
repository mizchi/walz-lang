//! Test oracle for standard WAT; intentionally independent of the Walz frontend.
#[path = "selfhost/gc_contract.rs"]
mod gc_contract;
#[path = "selfhost/gc_reference.rs"]
mod gc_reference;
use serde::Deserialize;
use std::{env, error::Error, fs};
use wasm_encoder::{
    CodeSection, Encode, Function, FunctionSection, Instruction, Module, TypeSection, ValType,
};

#[derive(Deserialize)]
struct Op {
    op: u8,
    immediate: String,
}
#[derive(Deserialize)]
struct Raw {
    params: Vec<i32>,
    locals: Vec<i32>,
    result: i32,
    code: Vec<Op>,
}

fn ty(n: i32) -> Option<ValType> {
    match n {
        127 => Some(ValType::I32),
        126 => Some(ValType::I64),
        _ => None,
    }
}

fn raw_valid(raw: &Raw) -> Option<bool> {
    if raw.params.len() > 256 || raw.locals.len() > 1024 || raw.code.len() > 262144 {
        return None;
    }
    let params = raw
        .params
        .iter()
        .map(|n| ty(*n))
        .collect::<Option<Vec<_>>>()?;
    let locals = raw
        .locals
        .iter()
        .map(|n| ty(*n).map(|t| (1, t)))
        .collect::<Option<Vec<_>>>()?;
    let result = if raw.result == 64 {
        vec![]
    } else {
        vec![ty(raw.result)?]
    };
    let mut types = TypeSection::new();
    types.ty().function(params, result);
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut body = Function::new(locals);
    for op in &raw.code {
        let imm = op.immediate.parse::<i64>().ok()?;
        let mut bytes = vec![op.op];
        match op.op {
            2..=4 => {
                let t = u8::try_from(imm).ok()?;
                if !matches!(t, 64 | 126 | 127) {
                    return None;
                }
                bytes.push(t);
            }
            12 | 13 | 16 | 32..=34 => u32::try_from(imm).ok()?.encode(&mut bytes),
            65 => i32::try_from(imm).ok()?.encode(&mut bytes),
            66 => imm.encode(&mut bytes),
            0 | 1 | 5 | 11 | 15 | 26 | 27 | 69..=90 | 106..=118 | 124..=136 | 167 | 172 | 173 => {
                if imm != 0 {
                    return None;
                }
            }
            _ => return None,
        }
        body.raw(bytes);
    }
    body.instruction(&Instruction::End);
    let mut code = CodeSection::new();
    code.function(&body);
    let mut module = Module::new();
    module.section(&types).section(&functions).section(&code);
    Some(
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(&module.finish())
            .is_ok(),
    )
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() == 3 && args[0] == "--gc-canonical" {
        fs::write(&args[2], gc_contract::canonical(&fs::read(&args[1])?)?)?;
        return Ok(());
    }
    if args.len() == 3 && args[0] == "--gc-ir" {
        let inputs: Vec<gc_reference::Raw> = serde_json::from_slice(&fs::read(&args[1])?)?;
        fs::write(
            &args[2],
            serde_json::to_vec(&inputs.iter().map(gc_reference::valid).collect::<Vec<_>>())?,
        )?;
        return Ok(());
    }
    if args.len() == 3 && args[0] == "--ir" {
        let inputs: Vec<Raw> = serde_json::from_slice(&fs::read(&args[1])?)?;
        let results = inputs
            .iter()
            .map(|raw| raw_valid(raw).unwrap_or(false))
            .collect::<Vec<_>>();
        fs::write(&args[2], serde_json::to_vec(&results)?)?;
        return Ok(());
    }
    if args.len() != 2 {
        return Err("usage: selfhost-reference input.wat output.wasm".into());
    }
    let wasm = wat::parse_file(&args[0])?;
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&wasm)?;
    fs::write(&args[1], wasm)?;
    Ok(())
}
