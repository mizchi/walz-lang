//! Development tool; not part of the language API or runtime.
use std::{collections::BTreeMap, error::Error as StdError, path::Path};
use walz::ir;

// The shared parser's diagnostic constructor is private to its owning crate.
// This development tool needs only a location and message, not the CLI API.
#[derive(Debug)]
struct Error(String);
impl Error {
    fn at(_: &str, span: ir::Span, message: impl std::fmt::Display) -> Self {
        Self(format!("byte {}: {message}", span.start))
    }
    fn with_code(self, code: &str) -> Self {
        Self(format!("{code}: {}", self.0))
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl StdError for Error {}

#[allow(dead_code)]
#[path = "../src/sexpr.rs"]
mod sexpr;

// This source catalogue preserves declarations and source spelling; compilation
// performs contextual elaboration through the library's real applications pass.
mod applications {
    pub(crate) fn expand(_: &str, _: &mut crate::sexpr::Sexp, _: bool) -> Result<(), crate::Error> {
        Ok(())
    }
}
#[path = "../src/standard_sources.rs"]
mod standard_sources;

fn print(node: &sexpr::Sexp, source: &str, names: &BTreeMap<String, String>) -> String {
    if let Some(atom) = node.atom() {
        return names.get(atom).cloned().unwrap_or_else(|| atom.into());
    }
    if let Some(items) = node.list() {
        return format!(
            "({})",
            items
                .iter()
                .map(|n| print(n, source, names))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    source[node.span.start..node.span.end].into()
}

fn effect_text(e: &walz::effects::EffectSet, module: &ir::Module) -> String {
    let mut names = Vec::new();
    for (name, enabled) in [
        ("reads", e.reads),
        ("writes", e.writes),
        ("allocates", e.allocates),
        ("host", e.host),
        ("suspends", e.suspends),
    ] {
        if enabled {
            names.push(name.to_owned());
        }
    }
    for tag in &e.throws {
        names.push(format!("(throws {})", module.tags[tag.0].name));
    }
    if e.throws_unknown {
        names.push("throws-unknown".into());
    }
    if names.is_empty() {
        names.push("pure".into());
    }
    format!("(@effect {})", names.join(" "))
}

fn append_scope(
    node: &sexpr::Sexp,
    source: &str,
    namespace: &str,
    module: &ir::Module,
    inferred: &[walz::effects::EffectSet],
    out: &mut String,
) -> Result<(), Box<dyn StdError>> {
    let parts = node.list().ok_or("expected module or namespace")?;
    let declarations = if node.head() == Some("module") {
        &parts[1..]
    } else {
        &parts[2..]
    };
    let mut exports = BTreeMap::new();
    for item in declarations
        .iter()
        .filter(|n| matches!(n.head(), Some("@export" | "@public")))
    {
        for export in &item.list().unwrap()[1..] {
            let fields = export.list().ok_or("invalid export")?;
            exports.insert(
                fields[1].atom().ok_or("invalid export name")?.to_owned(),
                fields[0].atom().ok_or("invalid export kind")?,
            );
        }
    }
    let qualify = |name: &str| {
        if namespace.is_empty() {
            name.to_owned()
        } else {
            format!("{namespace}.{}", name.trim_start_matches('$'))
        }
    };
    let types: BTreeMap<_, _> = declarations
        .iter()
        .filter(|n| n.head() == Some("type"))
        .map(|n| {
            let name = n.list().unwrap()[1].atom().unwrap();
            (name.to_owned(), qualify(name))
        })
        .collect();
    for (name, kind) in exports {
        let public_name = qualify(&name);
        let declaration = declarations
            .iter()
            .find(|n| {
                n.list().and_then(|p| p.get(1)).and_then(sexpr::Sexp::atom) == Some(&name)
                    && matches!(n.head(), Some("func" | "type"))
            })
            .ok_or_else(|| format!("missing declaration for {public_name}"))?;
        out.push_str(&format!("### `{public_name}`\n\n"));
        if kind != "func" {
            out.push_str(&format!("import: `(type {public_name})`. "));
            if kind == "opaque" {
                out.push_str("Opaque public type. Its representation is private.\n\n");
                let constraints = declaration
                    .list()
                    .unwrap()
                    .iter()
                    .filter(|n| matches!(n.head(), Some("@type.params" | "@where")))
                    .collect::<Vec<_>>();
                if !constraints.is_empty() {
                    out.push_str(&format!("```text\n(type {public_name}\n"));
                    for clause in constraints {
                        out.push_str(&format!("  {}\n", print(clause, source, &types)));
                    }
                    out.push_str(")\n```\n\n");
                }
            } else {
                out.push_str("Public type with an exposed representation.\n\n```text\n");
                out.push_str(&print(declaration, source, &types));
                out.push_str("\n```\n\n");
            }
            continue;
        }
        let fields = declaration.list().unwrap();
        let generic = fields.iter().any(|n| n.head() == Some("@type.params"));
        let explicit = fields.iter().any(|n| n.head() == Some("@effect"));
        out.push_str(&format!("import: `(func {public_name})`. "));
        out.push_str(if explicit {
            "Effects are the declared upper bound; the body is checked.\n\n"
        } else if generic {
            "Effects are inferred at specialization, including the supplied closure effects.\n\n"
        } else {
            "Effects are inferred from the current body.\n\n"
        });
        out.push_str(&format!("```text\n(func {public_name}\n"));
        let signature = fields.iter().skip(2).filter(|n| {
            matches!(
                n.head(),
                Some("param" | "result" | "@type.params" | "@where" | "@effect")
            )
        });
        for field in signature {
            out.push_str(&format!("  {}\n", print(field, source, &types)));
        }
        if !explicit && !generic {
            let index = module
                .functions
                .iter()
                .position(|f| f.name == public_name)
                .ok_or_else(|| format!("missing checked function {public_name}"))?;
            out.push_str(&format!("  {}\n", effect_text(&inferred[index], module)));
        }
        out.push_str(")\n```\n\n");
    }
    for child in declarations
        .iter()
        .filter(|n| n.head() == Some("@namespace"))
    {
        let name = qualify(child.list().unwrap()[1].atom().unwrap());
        append_scope(child, source, &name, module, inferred, out)?;
    }
    Ok(())
}

fn document() -> Result<String, Box<dyn StdError>> {
    for (internal, _) in standard_sources::INTERNAL_LIBRARIES {
        if standard_sources::LIBRARIES
            .iter()
            .any(|(public, _)| public == internal)
        {
            return Err(format!("internal standard module in public catalogue: {internal}").into());
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = String::from(
        "# Core API types and effects\n\n<!-- Generated by just stdlib-docs. Do not edit signatures manually. -->\n\nGenerated from public declarations in `walz:core`, implemented in `walz/core/`. Update with `just stdlib-docs` and verify with `just check-stdlib-docs`.\nSelect names explicitly, for example `(@use \"walz:core\" (type $string) (func $string.u64))`.\nApply generics with `(@inst $name T...)`. Omitting a result declaration means unit.\n\n`pure` means an empty semantic effect set, not a guarantee of trap freedom, termination, or zero physical allocation.\nAn unannotated generic has no declared effect restriction; each specialization is checked for types, effects, and ownership.\n\n## Built-in types\n\nSee the [core module](standard.md) for type properties and ownership.\n\n| Import | Type parameters | Application |\n| --- | --- | --- |\n",
    );
    let types = sexpr::parse(standard_sources::TYPES)?;
    for alias in types
        .list()
        .unwrap()
        .iter()
        .filter(|n| n.head() == Some("@type.alias"))
    {
        let fields = alias.list().unwrap();
        let name = fields[1].atom().unwrap();
        let parameters = fields
            .iter()
            .find(|n| n.head() == Some("@type.params"))
            .map(|n| {
                n.list().unwrap()[1..]
                    .iter()
                    .map(|n| n.atom().unwrap())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let application = if parameters.is_empty() {
            name.to_owned()
        } else {
            format!("(@inst {name} {})", parameters.join(" "))
        };
        out.push_str(&format!(
            "| `(type {name})` | {} | `{application}` |\n",
            if parameters.is_empty() {
                "—".into()
            } else {
                format!("`{}`", parameters.join(" "))
            }
        ));
    }
    for (file, source) in standard_sources::LIBRARIES {
        out.push_str(&format!(
            "\n## `{file}`\n\nImplementation: [walz/core/{file}](../walz/core/{file}).\n\n"
        ));
        let loaded = walz::source::load(root.join("walz/core").join(file))?;
        let module = walz::check(loaded.text())?;
        let inferred = walz::effects::infer(&module);
        append_scope(
            &sexpr::parse(source)?,
            source,
            "",
            &module,
            &inferred,
            &mut out,
        )?;
    }
    Ok(format!("{}\n", out.trim_end()))
}

fn main() -> Result<(), Box<dyn StdError>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = root.join("docs/stdlib-api.md");
    let text = document()?;
    if std::env::args().nth(1).as_deref() == Some("--check") {
        if std::fs::read_to_string(output)? != text {
            return Err("standard API documentation is stale; run just stdlib-docs".into());
        }
    } else {
        std::fs::write(output, text)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_api_has_complete_signatures_bounds_and_effects_without_private_layouts() {
        let text = document().unwrap();
        for required in [
            "$future",
            "$result",
            "$string-builder",
            "(func $string-builder.new",
            "(param $s $string)",
            "(@effect allocates)",
            "(func $list.map",
            "(type $dict\n  (@type.params $T)\n  (@where (shared $T) (nonunit $T))",
            "(func $dict.remove",
            "(func $list-builder.finish",
            "(@type.params $T $U $F)",
            "(callable $F",
            "inferred at specialization",
            "(func $json.parse",
        ] {
            assert!(text.contains(required), "missing {required}");
        }
        for private in [
            "$Chunk",
            "(func $bytes.matches",
            "(func $json-string.quote",
            "(struct (field",
        ] {
            assert!(!text.contains(private), "leaked {private}");
        }
    }

    #[test]
    fn checked_in_document_matches_current_exports_and_signatures() {
        let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/stdlib-api.md");
        assert_eq!(
            std::fs::read_to_string(output).unwrap(),
            document().unwrap()
        );
    }
}
