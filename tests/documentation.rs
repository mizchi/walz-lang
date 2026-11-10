use std::path::{Path, PathBuf};
use std::process::Command;

fn successful(command: &mut Command) -> String {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn run(wasm: &Path, export: &str) -> String {
    successful(
        Command::new("wasmtime")
            .args([
                "run",
                "-W",
                "gc=y,function-references=y,exceptions=y",
                "--invoke",
                export,
            ])
            .arg(wasm),
    )
    .trim()
    .into()
}

#[test]
fn introductory_guides_compile_run_and_support_the_documented_project_workflow() {
    let scratch = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut executed = 0;
    for guide in ["getting-started", "language-guide"] {
        let text = std::fs::read_to_string(root.join(format!("docs/{guide}.md"))).unwrap();
        let directory = scratch.path().join(guide);
        std::fs::create_dir_all(&directory).unwrap();
        let mut paths = Vec::<PathBuf>::new();
        for (i, block) in text.split("```wat\n").skip(1).enumerate() {
            let source = block.split("```").next().unwrap();
            assert!(
                source.contains("(module"),
                "{guide}: use text fences for syntax fragments"
            );
            let name = source
                .lines()
                .next()
                .and_then(|line| line.strip_prefix(";; file: "))
                .map(str::to_owned)
                .unwrap_or_else(|| format!("example-{i}.wlt"));
            let path = directory.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, source).unwrap();
            paths.push(path);
        }
        assert!(!paths.is_empty(), "{guide} needs executable examples");
        for (i, path) in paths.iter().enumerate() {
            let loaded = wlt::source::load(path).unwrap();
            let compiled =
                wlt::compile(loaded.text()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let wasm = directory.join(format!("example-{i}.wasm"));
            std::fs::write(&wasm, &compiled.wasm).unwrap();
            if let Some(module) = &compiled.ir {
                if module
                    .functions
                    .iter()
                    .any(|f| f.exports.iter().any(|name| name == "main"))
                {
                    assert_eq!(run(&wasm, "main"), "42", "{}", path.display());
                    executed += 1;
                }
                for case in &module.tests {
                    assert_ne!(run(&wasm, &case.export), "0", "{}", path.display());
                }
            }
        }
        if guide == "getting-started" {
            let cli = env!("CARGO_BIN_EXE_wlt");
            successful(Command::new(cli).current_dir(&directory).arg("build"));
            assert!(directory.join("target/main.wasm").is_file());
            assert_eq!(
                successful(Command::new(cli).current_dir(&directory).arg("run")).trim(),
                "42"
            );
            successful(Command::new(cli).current_dir(&directory).arg("test"));
            successful(
                Command::new(cli)
                    .current_dir(&directory)
                    .args(["bench", "--list"]),
            );
        }
    }
    assert!(executed >= 10, "keep the introductory patterns executable");
}

#[test]
fn entry_guides_link_to_existing_local_documents_and_examples() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for name in [
        "README.md",
        "README.ja.md",
        "docs/README.md",
        "docs/features.md",
        "docs/getting-started.md",
        "docs/language-guide.md",
        "docs/development.md",
    ] {
        let path = root.join(name);
        let text = std::fs::read_to_string(&path).unwrap();
        for link in text.split("](").skip(1) {
            let target = link.split(')').next().unwrap();
            if target.contains("://") || target.starts_with('#') {
                continue;
            }
            let file = target.split('#').next().unwrap();
            assert!(
                path.parent().unwrap().join(file).exists(),
                "{name}: broken link to {target}"
            );
        }
    }
}
