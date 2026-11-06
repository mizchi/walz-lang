#[test]
fn counted_ledger_compiles_without_repeated_state_types() {
    let path = "examples/apps/stock-ledger/main-counted.wlt";
    let source = wlt::source::load(path).unwrap();
    let compiled = wlt::compile(source.text()).unwrap();
    let module = compiled.ir.unwrap();
    let main = module.functions.iter().find(|f| f.name == "$main").unwrap();
    assert_eq!(module.type_name(&main.result), "(%result i32 i32)");
    assert_eq!(
        main.locals
            .iter()
            .filter(|l| l.name == "$initial-scan-state")
            .count(),
        1
    );
    let state = module
        .types
        .iter()
        .find(|t| t.name.ends_with(".State"))
        .unwrap();
    assert!(
        main.locals
            .iter()
            .any(|l| module.type_name(&l.ty) == format!("(%tuple (ref {}) i32)", state.name))
    );
    for alias in ["$ScanState", "$LineCount", "$IoError"] {
        assert!(!module.types.iter().any(|t| t.name == alias));
    }
}
