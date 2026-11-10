//! Source export traversal shared by build-time indexing and its validation.
use crate::sexpr::Sexp;
use std::collections::BTreeSet;

pub(crate) fn exports(node: &Sexp, namespace: &str, out: &mut BTreeSet<(String, String)>) {
    match node.head() {
        Some("@export" | "@public") => {
            for item in &node.list().unwrap()[1..] {
                let parts = item.list().unwrap();
                let kind = if item.head() == Some("opaque") {
                    "type"
                } else {
                    parts[0].atom().unwrap()
                };
                let name = parts[1].atom().unwrap();
                out.insert((
                    kind.into(),
                    if namespace.is_empty() {
                        name.into()
                    } else {
                        format!("${namespace}.{}", &name[1..])
                    },
                ));
            }
        }
        Some("module") => {
            for child in &node.list().unwrap()[1..] {
                exports(child, namespace, out);
            }
        }
        Some("@namespace") => {
            let parts = node.list().unwrap();
            let name = parts[1].atom().unwrap().trim_start_matches('$');
            let name = if namespace.is_empty() {
                name.into()
            } else {
                format!("{namespace}.{name}")
            };
            for child in &parts[2..] {
                exports(child, &name, out);
            }
        }
        _ => {}
    }
}
