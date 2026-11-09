#[path = "support/macro_fuzz.rs"]
mod macro_fuzz;

#[test]
fn seeded_user_macros_agree_with_manual_expansion_model_and_engines() {
    macro_fuzz::run();
}

#[test]
fn shrinking_retains_the_failure_class_and_reduces_nested_payloads() {
    use macro_fuzz::Expr as E;
    let expr = E::Add(
        Box::new(E::Const(10)),
        Box::new(E::Dup(Box::new(E::Add(
            Box::new(E::Const(4)),
            Box::new(E::Trace(7)),
        )))),
    );
    let reduced = macro_fuzz::reduce(expr, |e| e.duplicates_trace());
    assert_eq!(reduced, E::Dup(Box::new(E::Trace(7))));
}

#[test]
fn expanded_and_manual_inputs_enforce_equal_static_obligations() {
    macro_fuzz::static_obligations();
}
