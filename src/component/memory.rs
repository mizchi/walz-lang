//! A generated adapter's linear memory is explicit, independently of the
//! implementation's data memories. Only compiler-owned folded instructions are
//! qualified here; source instructions keep their checked memory references.
pub(crate) const NAME: &str = "$__abi_memory";
pub(crate) fn qualify(generated: &str) -> String {
    let source = format!("(module {generated})");
    let root = crate::sexpr::parse(&source).expect("compiler-generated folded WAT");
    let mut pending = vec![&root];
    let mut edits = vec![];
    while let Some(n) = pending.pop() {
        if let Some(p) = n.list() {
            if let Some(head) = n.head() {
                let scalar = head.split_once('.').is_some_and(|(ty, op)| {
                    matches!(ty, "i32" | "i64" | "f32" | "f64" | "v128")
                        && (op.starts_with("load") || op.starts_with("store"))
                });
                let count = if head == "memory.copy" {
                    2
                } else if scalar || matches!(head, "memory.size" | "memory.grow" | "memory.fill") {
                    1
                } else {
                    0
                };
                if count > 0 {
                    edits.push((p[0].span.end, format!(" {}", vec![NAME; count].join(" "))));
                }
            }
            pending.extend(p);
        }
    }
    let mut out = source;
    edits.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    for (at, text) in edits {
        out.insert_str(at, &text);
    }
    out[8..out.len() - 1].into()
}
