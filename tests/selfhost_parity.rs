#[path = "../tools/selfhost/contract.rs"]
mod contract;

fn canonical(source: &str) -> Vec<u8> {
    contract::canonical(&wat::parse_str(source).unwrap()).unwrap()
}

#[test]
fn canonical_module_ignores_names_type_interning_and_local_grouping() {
    use wasm_encoder::*;
    let reference = canonical(
        "(module (func $a (export \"a\") (param $x i32) (result i32) (local $p i32) (local $q i32) (local.get $x)) (func $b (export \"b\") (param i32) (result i32) (local.get 0)))",
    );
    let mut types = TypeSection::new();
    types.ty().function([ValType::I32], [ValType::I32]);
    types.ty().function([ValType::I32], [ValType::I32]);
    let mut functions = FunctionSection::new();
    functions.function(0).function(1);
    let mut exports = ExportSection::new();
    exports
        .export("a", ExportKind::Func, 0)
        .export("b", ExportKind::Func, 1);
    let mut code = CodeSection::new();
    for locals in [vec![(1, ValType::I32), (1, ValType::I32)], vec![]] {
        let mut f = Function::new(locals);
        f.instruction(&Instruction::LocalGet(0))
            .instruction(&Instruction::End);
        code.function(&f);
    }
    let mut module = Module::new();
    module
        .section(&types)
        .section(&functions)
        .section(&exports)
        .section(&code);
    assert_eq!(reference, contract::canonical(&module.finish()).unwrap());
}

#[test]
fn canonical_module_preserves_all_executable_details_and_export_order() {
    let reference =
        canonical("(module (func (export \"a\") (export \"b\") (result i32) (i32.const 42)))");
    for changed in [
        "(module (func (export \"a\") (export \"b\") (result i32) (i32.const 43)))",
        "(module (func (export \"b\") (export \"a\") (result i32) (i32.const 42)))",
        "(module (func (export \"a\") (export \"b\") (result i64) (i64.const 42)))",
    ] {
        assert_ne!(reference, canonical(changed));
    }
    assert_ne!(
        canonical("(module (func (unreachable) (drop (i32.const 1))))"),
        canonical("(module (func (unreachable) (drop (i32.const 2))))")
    );
}

#[test]
fn parity_contract_rejects_invalid_or_out_of_scope_modules() {
    for source in [
        "(module (memory 1))",
        "(module (import \"h\" \"f\" (func)))",
        "(module (func (result f64) (f64.const 1)))",
        "(module (func (result i32 i32) (i32.const 1) (i32.const 2)))",
        "(module (func (result i64) (i32.const 1)))",
        "(module (func (i32.clz (i32.const 1)) drop))",
    ] {
        assert!(
            contract::canonical(&wat::parse_str(source).unwrap()).is_err(),
            "{source}"
        );
    }
    let mut module = wasm_encoder::Module::new();
    module.section(&wasm_encoder::CustomSection {
        name: "unknown".into(),
        data: (&b"x"[..]).into(),
    });
    assert!(contract::canonical(&module.finish()).is_err());
}
