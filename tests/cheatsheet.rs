use std::process::Command;

#[test]
fn cheatsheet_complete_modules_compile_and_their_tests_execute() {
    let guide = include_str!("../CHEATSHEET.md");
    let modules = tempfile::tempdir().unwrap();
    let mut examples = 0;
    for block in guide.split("```wat\n").skip(1) {
        let source = block.split("```").next().unwrap();
        // Printed includes resolve from the project root, as in the guide.
        let input = tempfile::Builder::new()
            .prefix(".cheatsheet-")
            .suffix(".walz")
            .tempfile_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap();
        let named = source.lines().next().and_then(|line| {
            line.strip_prefix(";; ")
                .and_then(|line| line.split(':').next())
                .filter(|name| name.ends_with(".walz"))
        });
        let named_path = named.map(|name| modules.path().join(name));
        let path = named_path.as_deref().unwrap_or(input.path());
        std::fs::write(path, source).unwrap();
        let loaded = walz::source::load(path).unwrap();
        let compiled = walz::compile(loaded.text())
            .unwrap_or_else(|e| panic!("cheatsheet example {}: {e}", examples + 1));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("example.wasm");
        std::fs::write(&path, compiled.wasm).unwrap();
        for test in &compiled.ir.as_ref().unwrap().tests {
            let output = Command::new("wasmtime")
                .args([
                    "run",
                    "-W",
                    "gc=y,function-references=y,exceptions=y",
                    "--invoke",
                    &test.export,
                ])
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_ne!(String::from_utf8_lossy(&output.stdout).trim(), "0");
        }
        examples += 1;
    }
    assert!(
        examples >= 3,
        "keep at least the function, enum and Result examples executable"
    );
}
