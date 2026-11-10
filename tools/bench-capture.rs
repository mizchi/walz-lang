//! Internal artifact writer for runtime/bench-tune.mjs. Never overwrite a run.
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: bench-capture source.walz output-directory".into());
    }
    let source = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    let loaded = walz::source::load(&source)?;
    let module = walz::check(loaded.text()).map_err(|e| loaded.diagnostic(e))?;
    if module.benches.is_empty() {
        return Err("no benchmarks in source".into());
    }
    std::fs::create_dir_all(output.parent().ok_or("output needs a parent directory")?)?;
    std::fs::create_dir(&output)?;
    std::fs::copy(&source, output.join("source.walz"))?;
    std::fs::write(output.join("expanded.walz"), loaded.text())?;
    let mut cases = Vec::new();
    for (index, bench) in module.benches.iter().enumerate() {
        let file = format!("case-{index}.wasm");
        std::fs::write(
            output.join(&file),
            walz::benchmark::compile(loaded.text(), &module, index)
                .map_err(|e| loaded.diagnostic(e))?,
        )?;
        cases.push(serde_json::json!({"name": bench.name, "file": file}));
    }
    std::fs::write(
        output.join("metadata.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "walz-bench-snapshot-v1", "source": source.canonicalize()?, "cases": cases,
        }))?,
    )?;
    Ok(())
}
