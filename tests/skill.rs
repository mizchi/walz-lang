use std::path::Path;

#[test]
fn portable_skill_examples_are_checked_walz_programs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".agents/skills/walz");
    let mut examples = 0;
    for file in ["references/syntax.md", "references/cli.md"] {
        let text = std::fs::read_to_string(root.join(file)).unwrap();
        for (i, block) in text.split("```wat\n").skip(1).enumerate() {
            let source = block.split("```").next().unwrap();
            let compiled =
                walz::compile(source).unwrap_or_else(|e| panic!("{file} example {i}: {e}"));
            assert!(!compiled.wasm.is_empty());
            examples += 1;
        }
    }
    assert!(examples >= 3);
}
