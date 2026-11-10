use std::process::Command;

fn source(count: usize) -> String {
    let variants = (0..count)
        .map(|i| format!("(v{i} i32)"))
        .collect::<String>();
    let arms = (0..count)
        .rev()
        .map(|i| format!("(case v{i} ($x) (+ $x {i}))"))
        .collect::<String>();
    let makers = (0..count).map(|i| format!("(func $make{i} (export \"make{i}\") (param $x i32) (result (ref $E)) (@variant.new $E v{i} $x))")).collect::<String>();
    format!(
        "(module (type $E (@variant {variants})) (func $read (export \"read\") (param $x (ref $E)) (result i32) (@match $E (result i32) $x {arms})) {makers} (func $main (export \"main\") (result i32) (call $read (call $make{} (i32.const {})))))",
        count - 1,
        43 - count as i32
    )
}

fn execute(bytes: &[u8], export: &str, expected: Option<&str>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.wasm");
    std::fs::write(&path, bytes).unwrap();
    for (engine, args) in [
        (
            "wasmtime",
            vec![
                "run",
                "-W",
                "gc=y,function-references=y,exceptions=y",
                "--invoke",
                export,
            ],
        ),
        (
            "node",
            vec![
                "--input-type=module",
                "-e",
                "import fs from 'node:fs';const {instance}=await WebAssembly.instantiate(fs.readFileSync(process.argv[1]));console.log(String(instance.exports[process.argv[2]]()));",
            ],
        ),
    ] {
        let mut cmd = Command::new(engine);
        cmd.args(args).arg(&path);
        if engine == "node" {
            cmd.arg(export);
        }
        let output = cmd.output().unwrap();
        let error = String::from_utf8_lossy(&output.stderr);
        if let Some(expected) = expected {
            assert!(output.status.success(), "{engine}: {error}");
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
        } else {
            assert!(
                !output.status.success(),
                "{engine}: forged value was accepted"
            );
            assert!(
                error.contains("wasm trap") || error.contains("RuntimeError"),
                "{engine}: {error}"
            );
        }
    }
}

fn interop_wasm() -> Vec<u8> {
    let mut wat = walz::compile(&source(8)).unwrap().wat;
    wat.truncate(wat.rfind(')').unwrap());
    for i in 0..8 {
        wat.push_str(&format!(
            "(func (export \"case{i}\") (result i32) (call $read (call $make{i} (i32.const {}))))",
            42 - i
        ));
    }
    for (name, value) in [
        ("invalid-tag", "(struct.new $E (i32.const -1))"),
        ("upper-tag", "(struct.new $E (i32.const 8))"),
        ("base-only", "(struct.new $E (i32.const 0))"),
        (
            "wrong-case",
            "(struct.new $__variant_0_0 (i32.const 7) (i32.const 42))",
        ),
    ] {
        wat.push_str(&format!(
            "(func (export \"{name}\") (result i32) (call $read {value}))"
        ));
    }
    wat.push(')');
    wat::parse_str(&wat).unwrap()
}

fn check_interop(bytes: &[u8]) {
    for i in 0..8 {
        execute(bytes, &format!("case{i}"), Some("42"));
    }
    for name in ["invalid-tag", "upper-tag", "base-only", "wrong-case"] {
        execute(bytes, name, None);
    }
}

#[test]
fn large_dense_matches_use_one_tag_load_and_keep_every_case_cast() {
    for count in [8, 16, 32, 64] {
        for optimize in [false, true] {
            let built =
                walz::compile_with_options(&source(count), walz::CompileOptions { optimize })
                    .unwrap();
            assert!(
                built.wat.contains("br_table"),
                "{count} cases: {}",
                built.wat
            );
            assert_eq!(built.wat.matches("(struct.get $E 0 ").count(), 1);
            assert_eq!(built.wat.matches("(ref.cast ").count(), count);
            execute(&built.wasm, "main", Some("42"));
        }
    }
    assert!(!walz::compile(&source(2)).unwrap().wat.contains("br_table"));
}

#[test]
fn raw_wasm_callers_keep_tag_and_subtype_validation() {
    check_interop(&interop_wasm());
}

fn semantic_sources() -> Vec<String> {
    let variants = (0..8).map(|i| format!("(v{i} i32)")).collect::<String>();
    let subject = "(@variant.new $E v7 (i32.const 42))";
    let matcher = |selected: &str, other: &str| {
        format!(
            "(@match {subject} {})",
            (0..8)
                .rev()
                .map(|i| format!("(case v{i} ($x) {})", if i == 7 { selected } else { other }))
                .collect::<String>()
        )
    };
    let bodies = [
        matcher("$x", "(i32.const 0)"),
        matcher("(return $x)", "(i32.const 0)"),
        matcher("(return $x)", "(return (i32.const 0))"),
        matcher(&matcher("$x", "(i32.const 0)"), "(i32.const 0)"),
        format!(
            "(block $done (result i32) {})",
            matcher("(br $done $x)", "(i32.const 0)")
        ),
        format!(
            "(@try (result i32) (do {}) (catch $Err ($code) $code))",
            matcher("(throw $Err $x)", "(i32.const 0)")
        ),
        format!(
            "(block (result i32) {} (i32.const 42))",
            matcher("(nop)", "(nop)")
        ),
    ];
    let mut sources=bodies.into_iter().map(|body|format!("(module (tag $Err (param i32)) (type $E (@variant {variants})) (func $main (export \"main\") (result i32) {body}))")).collect::<Vec<_>>();
    let arms = (0..8)
        .map(|i| format!("(case v{i} ($x) (@box.new i32 $x))"))
        .collect::<String>();
    sources.push(format!("(module (@use \"walz:core\" (type $box)) (type $E (@variant {variants})) (func $main (export \"main\") (result i32) (@let $owner (type (@inst $box i32)) (@match {subject} {arms}) (@match (@move $owner) (case own ($n) $n)))))"));
    sources.push(include_str!("component-fixtures/match-dispatch.walz").into());
    sources
}

#[test]
fn dense_branches_preserve_return_labels_exceptions_unit_and_owned_results() {
    for source in semantic_sources() {
        for optimize in [false, true] {
            let built = walz::compile_with_options(&source, walz::CompileOptions { optimize })
                .unwrap_or_else(|error| panic!("{error}\n{source}"));
            execute(&built.wasm, "main", Some("42"));
        }
    }
}

fn nullary_wasm() -> Vec<u8> {
    let variants = (0..8).map(|i| format!("(v{i})")).collect::<String>();
    let arms = (0..8)
        .map(|i| format!("(case v{i} () (i32.const 42))"))
        .collect::<String>();
    let source = format!(
        "(module (type $E (@variant {variants})) (func $read (export \"read\") (param $x (ref $E)) (result i32) (@match $x {arms})))"
    );
    let mut wat = walz::compile(&source).unwrap().wat;
    assert_eq!(wat.matches("(drop (ref.cast ").count(), 8);
    wat.truncate(wat.rfind(')').unwrap());
    wat.push_str("(func (export \"good\") (result i32) (call $read (struct.new $__variant_0_7 (i32.const 7)))) (func (export \"bad\") (result i32) (call $read (struct.new $E (i32.const 7)))))");
    wat::parse_str(wat).unwrap()
}

#[test]
fn nullary_cases_still_validate_the_runtime_subtype() {
    let bytes = nullary_wasm();
    execute(&bytes, "good", Some("42"));
    execute(&bytes, "bad", None);
}

#[test]
#[ignore = "requires Binaryen 133; run just test-wasm-opt /path/to/wasm-opt"]
fn wasm_opt_open_world_preserves_values_and_traps() {
    let optimizer = std::env::var_os("WALZ_WASM_OPT").expect("set WALZ_WASM_OPT to wasm-opt 133");
    let version = Command::new(&optimizer).arg("--version").output().unwrap();
    assert!(String::from_utf8_lossy(&version.stdout).contains("version 133"));
    let mut programs = vec![interop_wasm(), nullary_wasm()];
    programs.extend(
        semantic_sources()
            .iter()
            .map(|s| walz::compile(s).unwrap().wasm),
    );
    programs.push(walz::compile("(module (func $main (export \"main\") (result i32) (@let $unused (i32.div_s (i32.const 1) (i32.const 0)) (i32.const 42))))").unwrap().wasm);
    let last = programs.len() - 1;
    for (i, bytes) in programs.iter().enumerate() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in.wasm");
        let output = dir.path().join("out.wasm");
        std::fs::write(&input, bytes).unwrap();
        let status = Command::new(&optimizer)
            .arg(&input)
            .args([
                "--enable-gc",
                "--enable-reference-types",
                "--enable-exception-handling",
                "-O3",
                "-o",
            ])
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stderr)
        );
        let bytes = std::fs::read(output).unwrap();
        if i == 0 {
            check_interop(&bytes);
        } else if i == 1 {
            execute(&bytes, "good", Some("42"));
            execute(&bytes, "bad", None);
        } else {
            execute(&bytes, "main", if i == last { None } else { Some("42") });
        }
    }
}
