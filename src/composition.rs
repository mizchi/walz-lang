//! Component composition is separate from statically linked Walz libraries.
//! Native component syntax remains authoritative; sugar produces checked
//! implementations, concrete interfaces and explicitly bound instances.
use crate::{
    Error,
    bindings::BindingPolicy,
    sexpr::{Kind, Sexp},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
mod connections;
mod contract;
pub(crate) mod embedded;
mod generics;
mod interface;
mod memory;
pub(crate) mod metadata;
pub(crate) mod queries;
mod source;
mod value_exports;
mod wit_print;
mod wit_projection;
pub use interface::{Interface, Item};
pub use memory::{CanonicalMemory, Memory, MemoryId, MemoryUse};

#[derive(Serialize)]
pub struct Inspection {
    pub interface: Interface,
    pub memories: Vec<MemoryUse>,
    pub canonical_memories: Vec<memory::CanonicalMemory>,
    pub unresolved_memories: Vec<String>,
}
pub struct Component {
    pub wasm: Vec<u8>,
    pub wat: String,
    pub wit: String,
    pub interface: Interface,
    pub manifest: serde_json::Value,
}
fn error(message: impl std::fmt::Display) -> Error {
    Error::message(format!("component source: {message}")).with_code("E_COMPONENT_SOURCE")
}
pub fn is_component(source: &str) -> bool {
    // Native WAT remains the reader for the standard language.
    crate::sexpr::parse(source).is_ok_and(|r| r.head() == Some("component"))
}
pub(crate) fn validate_library(source: &str, root: &Sexp) -> Result<(), Error> {
    for node in root.list().unwrap_or(&[]).iter().skip(1) {
        if matches!(node.head(), Some("export" | "start" | "data" | "global"))
            || matches!(node.head(), Some("func" | "memory" | "table"))
                && node
                    .list()
                    .unwrap()
                    .iter()
                    .skip(1)
                    .any(|n| n.head() == Some("export"))
        {
            return Err(Error::at(source,node.span,"@lib has no runtime initialization or ABI exports; select its API with @export and expose it from an entry").with_code("E_LIBRARY_ABI"));
        }
    }
    Ok(())
}
pub(crate) fn reject_library_entry(source: &str) -> Result<(), Error> {
    if let Ok(root) = crate::sexpr::parse(source)
        && root.head() == Some("@lib")
    {
        validate_library(source, &root)?;
        return Err(Error::message(
            "a library has no executable ABI; build a module or component entry".into(),
        )
        .with_code("E_LIBRARY_ENTRY"));
    }
    Ok(())
}
pub(crate) fn into_compilation(c: Component) -> crate::Compilation {
    crate::Compilation {
        wasm: c.wasm,
        wat: c.wat,
        ir: None,
        analysis: crate::Analysis {
            opaque: true,
            module_effects: crate::Effects::unknown(),
            functions: vec![],
        },
    }
}
pub fn inspect(bytes: &[u8]) -> Result<Inspection, Error> {
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(bytes)
        .map_err(error)?;
    if !wasmparser::Parser::is_component(bytes) {
        return Err(error("expected Component Wasm"));
    }
    let interface = Interface::decode(bytes, None)?;
    let text = wasmprinter::print_bytes(bytes).map_err(error)?;
    let graph = memory::inspect(&text)?;
    Ok(Inspection {
        interface,
        memories: graph.uses,
        canonical_memories: graph.canonical,
        unresolved_memories: graph.unresolved,
    })
}
pub fn compile(source: &str, policy: &BindingPolicy) -> Result<Component, Error> {
    compile_inner(source, policy, &mut generics::Budget::default(), 0)
}
fn compile_inner(
    source: &str,
    policy: &BindingPolicy,
    budget: &mut generics::Budget,
    depth: usize,
) -> Result<Component, Error> {
    let root = crate::sexpr::parse(source)?;
    if root.head() != Some("component") {
        return Err(error("expected (component ...)"));
    }
    if depth >= 64 {
        return Err(error("Component expansion nesting limit exceeded"));
    }
    if let Some(expanded) = generics::expand(source, &root, budget, policy, depth)? {
        let root = crate::sexpr::parse(&expanded.text)?;
        source::compile(&expanded.text, &root, policy, budget, depth, true)
            .map_err(|e| expanded.annotate(source, e))
    } else {
        source::compile(source, &root, policy, budget, depth, false)
    }
}
fn assemble(
    mut text: String,
    world: Option<(&str, &str)>,
    require_wit: bool,
    embedded: &embedded::Registry,
) -> Result<Component, Error> {
    if let Some((package, name)) = world {
        let metadata = serde_json::json!({"package":package,"world":name}).to_string();
        let root = crate::sexpr::parse(&text)?;
        let p = root.list().unwrap();
        let at = p
            .get(1)
            .filter(|n| n.atom().is_some_and(|s| s.starts_with('$')))
            .map_or(p[0].span.end, |n| n.span.end);
        text.insert_str(
            at,
            &format!(
                " (@custom \"walz:world-v1\" {}) ",
                crate::binary::quoted(metadata.as_bytes())
            ),
        );
    }
    let parsed = wat::parse_str(&text).map_err(error)?;
    let wasm = embedded::resolve(&parsed, embedded)?;
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
        .validate_all(&wasm)
        .map_err(error)?;
    metadata::validate(&wasm)?;
    interface::validate_metadata(&wasm)?;
    let interface = match Interface::decode(&wasm, world) {
        Ok(i) => i,
        Err(e) if !require_wit => Interface::opaque(e.to_string()),
        Err(e) => return Err(e),
    };
    let wit = interface.wit.clone();
    let normalized = wasmprinter::print_bytes(&wasm).map_err(error)?;
    let memories = memory::inspect(&normalized)?;
    let text = if wasm != parsed {
        wasmprinter::print_bytes(&wasm).map_err(error)?
    } else {
        text
    };
    let manifest = serde_json::json!({"version":2,"imports":[],"asyncExports":interface.exports.iter().filter(|i|i.asynchronous).map(|i|&i.name).collect::<Vec<_>>(),"factories":[],"composition":true,"interface":{"imports":interface.imports,"exports":interface.exports,"unsupported":interface.unsupported},"memoryGraph":memories});
    Ok(Component {
        wat: text,
        wasm,
        wit,
        interface,
        manifest,
    })
}
fn render(n: &Sexp) -> String {
    match &n.kind {
        Kind::Atom(s) => s.clone(),
        Kind::Text(s) => crate::binary::quoted(s.as_bytes()),
        Kind::Bytes(b) => crate::binary::quoted(b),
        Kind::List(xs) => format!("({})", xs.iter().map(render).collect::<Vec<_>>().join(" ")),
    }
}
