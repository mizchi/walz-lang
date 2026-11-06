use std::process::Command;

fn run(source: &str, expected: &str, trap: bool) {
    for optimize in [false, true] {
        let compiled = wlt::compile_with_options(source, wlt::CompileOptions { optimize }).unwrap();
        assert!(!compiled.analysis.opaque);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.wasm");
        std::fs::write(&path, compiled.wasm).unwrap();
        let output = Command::new("wasmtime")
            .args(["run", "--invoke", "main"])
            .arg(path)
            .output()
            .unwrap();
        if trap {
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
        } else {
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
        }
    }
}

#[test]
fn sub_mul_and_div_resolve_to_the_same_wasm_as_native_scalar_operations() {
    for ty in ["i32", "i64", "f32", "f64"] {
        for (symbol, suffix, a, b, expected) in [
            ("-", "sub", "44", "2", "42"),
            ("*", "mul", "6", "7", "42"),
            (
                "/",
                if ty.starts_with('i') { "div_s" } else { "div" },
                "-84",
                "2",
                "-42",
            ),
        ] {
            let source = format!(
                "(module (func $main (export \"main\") (result {ty}) (%effect pure) ({symbol} {a} {b})))"
            );
            let explicit = source.replace(
                &format!("({symbol} {a} {b})"),
                &format!("({ty}.{suffix} ({ty}.const {a}) ({ty}.const {b}))"),
            );
            for optimize in [false, true] {
                let options = wlt::CompileOptions { optimize };
                assert_eq!(
                    wlt::compile_with_options(&source, options).unwrap().wasm,
                    wlt::compile_with_options(&explicit, options).unwrap().wasm
                );
            }
            run(&source, expected, false);
        }
    }
}

#[test]
fn arithmetic_keeps_wasm_wrapping_division_rounding_and_traps() {
    for (ty, body, expected) in [
        ("i32", "(- -2147483648 1)", "2147483647"),
        ("i64", "(* 9223372036854775807 2)", "-2"),
        ("i32", "(/ -7 2)", "-3"),
        ("i64", "(/ 7 -2)", "-3"),
    ] {
        run(
            &format!("(module (func $main (export \"main\") (result {ty}) {body}))"),
            expected,
            false,
        );
    }
    for ty in ["i32", "i64"] {
        let minimum = if ty == "i32" {
            "-2147483648"
        } else {
            "-9223372036854775808"
        };
        for (body, expected) in [
            ("(/ 1 0)".into(), "divide by zero"),
            (format!("(/ {minimum} -1)"), "overflow"),
        ] {
            run(
                &format!("(module (func $main (export \"main\") (result {ty}) {body}))"),
                expected,
                true,
            );
        }
    }
    for ty in ["f32", "f64"] {
        run(
            &format!(
                "(module (func $main (export \"main\") (result i32) ({ty}.eq ({ty}.const inf) (/ ({ty}.const 1) 0))))"
            ),
            "1",
            false,
        );
        run(
            &format!(
                "(module (func $main (export \"main\") (result i32) ({ty}.ne (/ ({ty}.const 0) 0) (/ ({ty}.const 0) 0))))"
            ),
            "1",
            false,
        );
        let source = format!("(module (func $f (param $x {ty}) (result {ty}) (/ $x 0)))");
        assert!(
            !wlt::analysis::analyze(&wlt::check(&source).unwrap()).functions[0]
                .effects
                .may_trap
        );
    }
    let module = wlt::check("(module (func $f (param $x i32) (result i32) (/ $x 0)))").unwrap();
    assert!(
        wlt::analysis::analyze(&module).functions[0]
            .effects
            .may_trap
    );
}

#[test]
fn arithmetic_constraints_work_in_generic_functions_and_reject_missing_instances() {
    for (class, op) in [("Sub", "-"), ("Mul", "*"), ("Div", "/")] {
        let source = format!("(module (func $calc (%type.params $T) (%where ({class} $T)) (param $a $T) (param $b $T) (result $T) ({op} $a $b))
          (func $main (export \"main\") (result i64) (call (%inst $calc i64) 84 2)))");
        run(
            &source,
            match op {
                "-" => "82",
                "*" => "168",
                _ => "42",
            },
            false,
        );
        for ty in ["v128", "%text", "(%own i32)", "%unit"] {
            let source = format!(
                "(module (func $require (%type.params $T) (%where ({class} $T)) (nop)) (func $unused (call (%inst $require {ty}))))"
            );
            assert_eq!(
                wlt::check(&source).unwrap_err().diagnostic().code,
                "E_GENERIC_BOUND"
            );
        }
    }
    for source in [
        "(module (func $f (result i32) (%let $x (* 6 7) $x)))",
        "(module (func $f (result i64) (/ (i32.const 84) (i64.const 2))))",
        "(module (type $N (%newtype i32)) (func $f (param $x $N) (result $N) (* $x $x)))",
        "(module (func $f (result i32) (/ 1)))",
    ] {
        assert!(wlt::compile(source).is_err(), "{source}");
    }
}
