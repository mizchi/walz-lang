//! Read-only Core Wasm inventory; source attribution is supplied separately.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .ok_or("usage: code-inventory input.wasm [shared-output.wasm]")?;
    let bytes = std::fs::read(path)?;
    let report = walz::code_sharing::inventory(&bytes)?;
    if let Some(output) = args.next() {
        let shared = walz::code_sharing::share_exact(&bytes)?;
        use std::io::Write;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?
            .write_all(&shared)?;
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
