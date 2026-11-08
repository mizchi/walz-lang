//! Link selected functions from an independently validated, closed component.
//! Each canonical path identifies one component instance per source identity.
use crate::ir::*;

pub(super) fn render(
    module: &Module,
    namespace: &str,
    group: usize,
    functions: &[(usize, &Function)],
) -> Option<String> {
    let dependency = module
        .components
        .iter()
        .find(|d| d.namespace == namespace)?;
    // Reserve its component index, then insert the exact validated binary.
    let mut text = format!(
        "(component $binary{group} (@custom \"wlt:embedded-component\" {:?}))\n(instance $binary_instance{group} (instantiate $binary{group}))\n",
        dependency.namespace
    );
    for (i, function) in functions {
        let FunctionKind::Import { name, .. } = &function.kind else {
            unreachable!()
        };
        let index = name
            .strip_prefix("export-")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let export = &dependency.exports[index];
        let mut instance = format!("$binary_instance{group}");
        for (depth, path) in export.path.iter().enumerate() {
            if depth + 1 == export.path.len() {
                text += &format!("(alias export {instance} {path:?} (func $host{i}))\n");
            } else {
                let alias = format!("$binary_api{i}_{depth}");
                text += &format!("(alias export {instance} {path:?} (instance {alias}))\n");
                instance = alias;
            }
        }
        text += &format!(
            "(core func $host_lower{i} (canon lower (func $host{i}) (memory (core memory $mem \"memory\")) (realloc (core func $mem \"realloc\"))))\n"
        );
    }
    Some(text)
}

pub(super) fn assemble(module: &Module, text: String) -> Result<(String, Vec<u8>), crate::Error> {
    let fail = |e: String| {
        crate::Error::message(format!("component assembly: {e}")).with_code("E_WASM_USE")
    };
    let bytes = wat::parse_str(&text).map_err(|e| fail(e.to_string()))?;
    if module.components.is_empty() {
        return Ok((text, bytes));
    }
    let mut output = wasm_encoder::Component::new();
    let mut depth = 0;
    let mut linked = std::collections::BTreeSet::new();
    for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
        let payload = payload.map_err(|e| fail(e.to_string()))?;
        if depth == 1
            && let Some((id, range)) = payload.as_section()
        {
            let original = &bytes[range.start as usize..range.end as usize];
            let mut data = original;
            if matches!(payload, wasmparser::Payload::ComponentSection { .. }) {
                for child in wasmparser::Parser::new(0).parse_all(original) {
                    if let wasmparser::Payload::CustomSection(section) =
                        child.map_err(|e| fail(e.to_string()))?
                        && section.name() == "wlt:embedded-component"
                    {
                        let namespace =
                            std::str::from_utf8(section.data()).map_err(|e| fail(e.to_string()))?;
                        let dependency = module
                            .components
                            .iter()
                            .find(|d| d.namespace == namespace)
                            .ok_or_else(|| fail("unknown embedded component".into()))?;
                        if !linked.insert(namespace.to_owned()) {
                            return Err(fail("duplicate embedded component".into()));
                        }
                        data = &dependency.bytes;
                        break;
                    }
                }
            }
            output.section(&wasm_encoder::RawSection { id, data });
        }
        match payload {
            wasmparser::Payload::Version { .. } => depth += 1,
            wasmparser::Payload::End(_) => depth -= 1,
            _ => {}
        }
    }
    if linked.len() != module.components.len() {
        return Err(fail("unresolved component dependency".into()));
    }
    let wasm = output.finish();
    let text = wasmprinter::print_bytes(&wasm).map_err(|e| fail(e.to_string()))?;
    Ok((text, wasm))
}
