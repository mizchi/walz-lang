//! Generate only immutable source metadata. User programs are still fully checked.
use std::{collections::BTreeMap, error::Error as StdError, path::Path};

mod ir {
    #[derive(Clone, Copy, Debug)]
    pub struct Span {
        pub start: usize,
        pub end: usize,
    }
}
#[derive(Debug)]
struct Error(String);
impl Error {
    fn at(_: &str, span: ir::Span, message: impl std::fmt::Display) -> Self {
        Self(format!("{}..{}: {message}", span.start, span.end))
    }
    fn with_code(self, _: &str) -> Self {
        self
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl StdError for Error {}

#[path = "src/standard_catalogue.rs"]
mod catalogue;
#[allow(dead_code)]
#[path = "src/sexpr.rs"]
mod sexpr;
#[path = "src/standard_sources.rs"]
mod sources;

fn main() -> Result<(), Box<dyn StdError>> {
    for file in [
        "src/sexpr.rs",
        "src/standard_catalogue.rs",
        "src/standard_sources.rs",
        "walz/core/types.walz",
    ] {
        println!("cargo:rerun-if-changed={file}");
    }
    let mut members = BTreeMap::new();
    for (file, source) in sources::LIBRARIES {
        println!("cargo:rerun-if-changed=walz/core/{file}");
        let mut exports = Default::default();
        catalogue::exports(&sexpr::parse(source)?, "", &mut exports);
        for (kind, name) in exports {
            assert!(
                members.insert((kind, name), *file).is_none(),
                "duplicate public standard member"
            );
        }
    }
    for (file, source) in sources::INTERNAL_LIBRARIES {
        println!("cargo:rerun-if-changed=walz/core/{file}");
        assert!(
            !sources::LIBRARIES.iter().any(|(name, _)| name == file),
            "internal module in public catalogue"
        );
        sexpr::parse(source)?;
    }
    let mut output = String::from("pub(crate) const MEMBERS: &[(&str, &str, &str)] = &[\n");
    for ((kind, name), file) in members {
        output.push_str(&format!("({kind:?}, {name:?}, {file:?}),\n"));
    }
    output.push_str("];\npub(crate) const CORE_ALIASES: &[(&str, usize, usize)] = &[\n");
    let types = sexpr::parse(sources::TYPES)?;
    let mut exported_types = Default::default();
    catalogue::exports(&types, "", &mut exported_types);
    for node in types
        .list()
        .unwrap()
        .iter()
        .skip(1)
        .filter(|n| n.head() == Some("@type.alias"))
    {
        let name = node.list().unwrap()[1].atom().unwrap();
        if !exported_types.contains(&("type".to_owned(), name.to_owned())) {
            continue;
        }
        output.push_str(&format!(
            "({name:?}, {}, {}),\n",
            node.span.start, node.span.end
        ));
    }
    output.push_str("];\n");
    std::fs::write(
        Path::new(&std::env::var("OUT_DIR")?).join("standard-index.rs"),
        output,
    )?;
    Ok(())
}
