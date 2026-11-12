//! Experimental harness kept separate from the stable CLI/Component ABI.
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        return Err("usage: js-native-build source.walz output-directory utf8|js-strings".into());
    }
    let mode = match args[2].as_str() {
        "utf8" => walz::component::js_native::Mode::Utf8,
        "js-strings" => walz::component::js_native::Mode::JsStrings,
        _ => return Err("unknown native string mode".into()),
    };
    let source = walz::source::load(&args[0])?;
    let built = walz::component::js_native::compile(source.text(), mode)?;
    let output = Path::new(&args[1]);
    std::fs::create_dir_all(output)?;
    std::fs::write(output.join("native.wasm"), built.wasm)?;
    std::fs::write(output.join("native.wat"), built.wat)?;
    std::fs::write(output.join("bindings.mjs"), built.bindings)?;
    std::fs::write(
        output.join("manifest.json"),
        serde_json::to_string_pretty(&built.manifest)?,
    )?;
    std::fs::write(
        output.join("runtime.mjs"),
        include_str!("../runtime/js-native.mjs"),
    )?;
    std::fs::write(output.join("package.json"), "{\"type\":\"module\"}\n")?;
    std::fs::write(
        output.join("index.js"),
        format!(
            "import {{instantiate as create}} from './runtime.mjs';\nimport {{bind}} from './bindings.mjs';\nconst manifest={};\nexport const instantiate=(options={{}})=>create(import.meta.url,bind,manifest,options);\n",
            built.manifest
        ),
    )?;
    Ok(())
}
