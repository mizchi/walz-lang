//! Persistent release-mode benchmark peer. JSON transport is outside clocks.
#[path = "selfhost/contract.rs"]
mod contract;

use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    error::Error,
    hint::black_box,
    io::{self, BufRead, Write},
    time::Instant,
};

#[derive(Deserialize)]
struct Request {
    cmd: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    wasm: Vec<u8>,
    #[serde(default)]
    operation: String,
    #[serde(default)]
    iterations: u32,
}
struct Case {
    source: String,
    wasm: Vec<u8>,
}

fn assemble(source: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let bytes = wat::parse_str(source)?;
    wasmparser::Validator::new().validate_all(&bytes)?;
    Ok(bytes)
}

fn handle(r: Request, cases: &mut BTreeMap<String, Case>) -> Result<Value, Box<dyn Error>> {
    match r.cmd.as_str() {
        "prepare" => {
            if cases.contains_key(&r.id) {
                return Err("duplicate case".into());
            }
            let structured = walz::check(&r.source).is_ok();
            walz::validate(&r.source)?;
            let wasm = walz::compile(&r.source)?.wasm;
            if assemble(&r.source)? != wasm {
                return Err("native WAT reference differs from compiler output".into());
            }
            let functions = walz::native::types(&wasm)?.functions.len();
            let canonical = contract::canonical(&wasm)?;
            let response = json!({"wasm":wasm, "canonical":canonical, "functions":functions, "structuredCheck":structured});
            cases.insert(
                r.id,
                Case {
                    source: r.source,
                    wasm,
                },
            );
            Ok(response)
        }
        "canonical" => Ok(json!({"canonical":contract::canonical(&r.wasm)?})),
        "measure" => {
            if !(1..=65536).contains(&r.iterations) {
                return Err("invalid iteration count".into());
            }
            let case = cases.get(&r.id).ok_or("unknown case")?;
            let mut total = 0u128;
            for _ in 0..r.iterations {
                match r.operation.as_str() {
                    "assemble" => {
                        let start = Instant::now();
                        let result = assemble(black_box(&case.source));
                        total += start.elapsed().as_nanos();
                        if result? != case.wasm {
                            return Err("measured WAT reference output changed".into());
                        }
                    }
                    "compile" => {
                        let start = Instant::now();
                        let result = walz::compile(black_box(&case.source));
                        total += start.elapsed().as_nanos();
                        let compiled = result?;
                        if compiled.wasm != case.wasm {
                            return Err("measured native output changed".into());
                        }
                        black_box(compiled);
                    }
                    "check" => {
                        let start = Instant::now();
                        let result = walz::validate(black_box(&case.source));
                        total += start.elapsed().as_nanos();
                        result?;
                    }
                    _ => return Err("unknown operation".into()),
                }
            }
            Ok(json!({"calls":r.iterations,"nsPerCall":total as f64 / r.iterations as f64}))
        }
        _ => Err("unknown request".into()),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut cases = BTreeMap::new();
    let mut stdout = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let response = serde_json::from_str::<Request>(&line?)
            .map_err(Into::into)
            .and_then(|r| handle(r, &mut cases));
        let response = match response {
            Ok(value) => json!({"ok":value}),
            Err(e) => json!({"error":e.to_string()}),
        };
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}
