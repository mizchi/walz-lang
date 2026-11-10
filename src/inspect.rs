//! Read-only source queries over the same checked IR used by compilation.
use crate::{
    Error, bindings::BindingPolicy, diagnostic::Label, ir::*, sexpr::Sexp, source::Source,
};
use serde::Serialize;
use std::{collections::BTreeMap, path::Path};
#[cfg(not(target_family = "wasm"))]
mod syntax_index;

/// Checked public JavaScript signatures and original export-token locations.
/// Source loading (including unsaved buffers) remains owned by the caller.
pub fn esm(loaded: &Source, policy: &BindingPolicy) -> Result<serde_json::Value, Error> {
    let mut metadata = crate::component::core_js::metadata_with_policy(loaded.text(), policy)
        .map_err(|e| loaded.diagnostic(e))?;
    fn locate(node: &Sexp, loaded: &Source, exports: &mut serde_json::Value) {
        // A template's export-shaped data is not a declaration in this source.
        // Only the checked expansion may introduce a real public binding.
        if node.head() == Some("%macro") {
            return;
        }
        if let Some(items) = node.list() {
            if node.head() == Some("export")
                && let Some(name) = items.get(1)
                && let Some(binding) = name.text().and_then(|s| exports.get_mut(s))
                && let Some(label) = loaded.label(
                    Span {
                        start: name.span.start + 1,
                        end: name.span.end - 1,
                    },
                    "export",
                )
            {
                binding["declaration"] = serde_json::to_value(label).unwrap();
            }
            for item in items {
                locate(item, loaded, exports);
            }
        }
    }
    locate(
        &crate::sexpr::parse(loaded.text())?,
        loaded,
        &mut metadata["exports"],
    );
    Ok(metadata)
}

#[derive(Debug, Serialize)]
pub struct TypeQuery {
    pub schema: &'static str,
    /// `wat` has no expression-level typed IR. Empty matches never imply a type.
    pub mode: &'static str,
    pub matches: Vec<TypeMatch>,
}

#[derive(Debug, Serialize)]
pub struct TypeMatch {
    pub function: String,
    pub specialization: Option<String>,
    /// Checked, expanded type. `source_type` is presentation metadata only.
    pub ty: String,
    pub source_type: Option<String>,
    pub location: Label,
    pub declaration: Option<Label>,
    pub instantiations: Vec<Label>,
}

struct Surface<'a> {
    functions: Vec<&'a Sexp>,
    annotations: BTreeMap<usize, String>,
}
impl Surface<'_> {
    fn annotation(&mut self, source: &str, name: &Sexp, ty: &Sexp) {
        let ty = if ty.head() == Some("type")
            && let Some([_, inner]) = ty.list()
        {
            inner
        } else {
            ty
        };
        self.annotations
            .insert(name.span.start, source[ty.span.start..ty.span.end].into());
    }
    fn collect<'a>(root: &'a Sexp, source: &str) -> Surface<'a> {
        fn walk<'a>(n: &'a Sexp, source: &str, out: &mut Surface<'a>) {
            let Some(p) = n.list() else { return };
            match n.head() {
                Some("func" | "%closure") => out.functions.push(n),
                Some("param" | "%capture" | "bind") if p.len() >= 3 => {
                    out.annotation(source, &p[1], &p[2])
                }
                Some("%let") if p.len() == 5 => out.annotation(source, &p[1], &p[2]),
                Some("%let") if p.len() == 3 => {
                    for binding in p[1].list().unwrap_or_default() {
                        if let Some([name, ty, _]) = binding.list() {
                            out.annotation(source, name, ty);
                        }
                    }
                }
                _ => {}
            }
            for child in p {
                walk(child, source, out);
            }
        }
        let mut result = Surface {
            functions: vec![],
            annotations: BTreeMap::new(),
        };
        walk(root, source, &mut result);
        result
    }
}

fn contains(span: Span, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}
fn token_at(node: &Sexp, offset: usize) -> bool {
    if !contains(node.span, offset) {
        return false;
    }
    if let Some(p) = node.list() {
        offset == node.span.start || p.iter().any(|n| token_at(n, offset))
    } else {
        true
    }
}

fn query(
    loaded: &Source,
    syntax: &Sexp,
    offsets: &[usize],
    module: &Module,
    instances: &[(Span, Span, String, String)],
) -> Vec<TypeMatch> {
    let source = loaded.text();
    let surface = Surface::collect(syntax, source);
    let mut results = Vec::new();
    for &offset in offsets {
        if !token_at(syntax, offset) {
            continue;
        }
        for function in &module.functions {
            let source_function = function.body.as_ref().and_then(|body| {
                surface
                    .functions
                    .iter()
                    .filter(|n| contains(n.span, body.span.start))
                    .min_by_key(|n| n.span.end - n.span.start)
            });
            let result_annotation = source_function.and_then(|n| {
                n.list()?
                    .iter()
                    .find(|n| n.head() == Some("result"))?
                    .list()?
                    .get(1)
            });
            let declaration = function.locals.iter().find(|l| {
                contains(l.span, offset)
                    && source.get(l.span.start..l.span.end) == Some(l.name.as_str())
            });
            let mut expression = function.body.as_ref().filter(|e| contains(e.span, offset));
            while let Some(inner) = expression.and_then(|e| {
                e.children()
                    .into_iter()
                    .filter(|e| contains(e.span, offset))
                    .min_by_key(|e| e.span.end - e.span.start)
            }) {
                expression = Some(inner);
            }
            let Some((ty, span, local)) = declaration
                .map(|l| (&l.ty, l.span, Some(l)))
                .or_else(|| {
                    expression.map(|e| {
                        let local = match e.kind {
                            ExprKind::Local(id) | ExprKind::Move(id) => {
                                Some(&function.locals[id.0])
                            }
                            _ => None,
                        };
                        (&e.ty, e.span, local)
                    })
                })
                .or_else(|| {
                    result_annotation
                        .filter(|n| contains(n.span, offset))
                        .map(|n| (&function.result, n.span, None))
                })
            else {
                continue;
            };
            let Some(location) = loaded.label(span, "type at this source location") else {
                continue;
            };
            let owner = surface
                .functions
                .iter()
                .filter(|n| contains(n.span, span.start))
                .min_by_key(|n| n.span.end - n.span.start);
            let function_name = owner
                .and_then(|n| n.list()?.get(1)?.atom())
                .unwrap_or(&function.name)
                .to_owned();
            let specialization = instances
                .iter()
                .find(|(_, _, _, name)| name == &function.name)
                .and_then(|(_, _, display, _)| {
                    display
                        .find('<')
                        .map(|i| format!("{function_name}{}", &display[i..]))
                });
            let instantiations = instances
                .iter()
                .filter(|(_, _, _, name)| name == &function.name)
                .filter_map(|(_, request, _, _)| {
                    loaded.label(
                        *request,
                        source
                            .get(request.start..request.end)
                            .unwrap_or("instantiation"),
                    )
                })
                .collect();
            results.push(TypeMatch {
                function: function_name,
                specialization,
                ty: module.type_name(ty),
                source_type: local
                    .and_then(|l| surface.annotations.get(&l.span.start).cloned())
                    .or_else(|| {
                        result_annotation
                            .filter(|n| contains(n.span, offset))
                            .map(|n| source[n.span.start..n.span.end].to_owned())
                    }),
                declaration: local.and_then(|l| loaded.label(l.span, "binding declared here")),
                location,
                instantiations,
            });
        }
    }
    results.sort_by(|a, b| {
        (
            &a.function,
            &a.specialization,
            &a.ty,
            &a.location.location.path,
            a.location.span.start,
        )
            .cmp(&(
                &b.function,
                &b.specialization,
                &b.ty,
                &b.location.location.path,
                b.location.span.start,
            ))
    });
    results.dedup_by(|a, b| {
        a.function == b.function
            && a.specialization == b.specialization
            && a.ty == b.ty
            && a.location.span.start == b.location.span.start
            && a.location.location.path == b.location.location.path
    });
    results
}

/// Query a loaded file at a one-based Unicode scalar position. `path=None`
/// queries the entry file; an imported file must belong to this snapshot.
/// Multiple concrete generic bodies are returned separately. This inspection
/// checks types/effects/ownership but does not prove laws, like `typing::report`.
pub fn type_at(
    loaded: &Source,
    path: Option<&Path>,
    line: usize,
    column: usize,
    policy: &BindingPolicy,
) -> Result<TypeQuery, Error> {
    let offsets = loaded.offsets(path, line, column)?;
    let syntax = crate::sexpr::parse(loaded.text()).map_err(|e| loaded.diagnostic(e))?;
    match crate::check_observed(loaded.text(), policy, |module, generics| {
        query(
            loaded,
            &syntax,
            &crate::macros::checked_offsets(&generics.macros, &offsets),
            module,
            &generics.instances,
        )
    }) {
        Ok((_, matches)) => Ok(TypeQuery {
            schema: "wlt-type-at-v1",
            mode: "typed",
            matches,
        }),
        Err(_) => {
            crate::compile_with_policy(loaded.text(), policy).map_err(|e| loaded.diagnostic(e))?;
            Ok(TypeQuery {
                schema: "wlt-type-at-v1",
                mode: "wat",
                matches: vec![],
            })
        }
    }
}

/// One immutable checked IR for editor queries. The protocol owns invalidation.
#[cfg(not(target_family = "wasm"))]
pub(crate) struct Snapshot {
    pub source: Source,
    pub error: Option<Error>,
    syntax: Sexp,
    index: syntax_index::Index,
    checked: Option<CheckedSnapshot>,
    links: Vec<(Span, crate::diagnostic::Label, String)>,
    macros: crate::macros::Report,
    performance_warnings: std::sync::OnceLock<Vec<crate::diagnostic::Diagnostic>>,
}
#[cfg(not(target_family = "wasm"))]
type CheckedSnapshot = (Module, Vec<(Span, Span, String, String)>);
#[cfg(not(target_family = "wasm"))]
impl Snapshot {
    pub fn new(source: Source, policy: &BindingPolicy) -> Result<Self, Error> {
        let syntax = crate::sexpr::parse(source.text()).map_err(|e| source.diagnostic(e))?;
        let index = syntax_index::Index::new(&source, &syntax, true);
        let mut links = vec![];
        let mut error = None;
        let mut macros = crate::macros::Report::default();
        let checked = match crate::check_observed(source.text(), policy, |module, generics| {
            macros = generics.macros.clone();
            fn defs(
                n: &Sexp,
                out: &mut BTreeMap<(crate::type_syntax::ReferenceKind, String), Span>,
            ) {
                let Some(p) = n.list() else { return };
                let kind = match n.head() {
                    Some("func" | "closure") => Some(crate::type_syntax::ReferenceKind::Function),
                    Some("type" | "type.alias" | "resource") => {
                        Some(crate::type_syntax::ReferenceKind::Type)
                    }
                    _ => None,
                };
                if let Some(kind) = kind
                    && let Some(name) = p.get(1).and_then(Sexp::atom)
                {
                    out.insert((kind, name.into()), p[1].span);
                }
                for c in p {
                    defs(c, out)
                }
            }
            let mut declarations = BTreeMap::new();
            defs(&generics.syntax, &mut declarations);
            let mut expanded = generics.syntax.clone();
            let _ = crate::type_syntax::map_references(&mut expanded, &mut |kind, n| {
                if let Some(name) = n.atom()
                    && let Some(span) = declarations.get(&(kind, name.into()))
                    && let Some(label) = source.label(*span, "definition")
                {
                    let display = if kind == crate::type_syntax::ReferenceKind::Function {
                        module
                            .functions
                            .iter()
                            .find(|f| f.name == name)
                            .map(|f| {
                                format!(
                                    "{} ({}) -> {}",
                                    f.name,
                                    f.params
                                        .iter()
                                        .map(|i| module.type_name(&f.locals[i.0].ty))
                                        .collect::<Vec<_>>()
                                        .join(", "),
                                    module.type_name(&f.result)
                                )
                            })
                            .unwrap_or_else(|| name.into())
                    } else {
                        name.into()
                    };
                    links.push((n.span, label, display));
                }
                Ok(())
            });
            generics.instances.clone()
        }) {
            Ok(checked) => Some(checked),
            Err(_) => {
                if let Err(e) = crate::compile_with_policy(source.text(), policy) {
                    error = Some(source.diagnostic(e));
                }
                None
            }
        };
        Ok(Self {
            source,
            error,
            syntax,
            index,
            checked,
            links,
            macros,
            performance_warnings: Default::default(),
        })
    }
    pub fn performance_warnings(&self) -> &[crate::diagnostic::Diagnostic] {
        self.performance_warnings.get_or_init(|| {
            self.checked
                .as_ref()
                .map(|(module, _)| {
                    crate::performance::analyze(self.source.text(), module)
                        .located(&self.source)
                        .warnings
                })
                .unwrap_or_default()
        })
    }
    pub fn query(&self, path: &Path, line: usize, column: usize) -> Result<TypeQuery, Error> {
        let offsets = self.source.offsets(Some(path), line, column)?;
        Ok(TypeQuery {
            schema: "wlt-type-at-v1",
            mode: if self.error.is_some() {
                "syntax"
            } else if self.checked.is_some() {
                "typed"
            } else {
                "wat"
            },
            matches: self
                .checked
                .as_ref()
                .map(|(m, i)| {
                    query(
                        &self.source,
                        &self.syntax,
                        &crate::macros::checked_offsets(&self.macros, &offsets),
                        m,
                        i,
                    )
                })
                .unwrap_or_default(),
        })
    }
    pub fn link(
        &self,
        path: &Path,
        line: usize,
        column: usize,
    ) -> Option<(&crate::diagnostic::Label, &str)> {
        let offsets = self.source.offsets(Some(path), line, column).ok()?;
        self.links
            .iter()
            .find(|(s, _, _)| offsets.iter().any(|o| contains(*s, *o)))
            .map(|(_, l, t)| (l, t.as_str()))
    }
    pub fn declaration(&self, path: &Path, line: usize, column: usize) -> Option<&Label> {
        self.index
            .definition(&self.source.offsets(Some(path), line, column).ok()?)
    }
    pub fn declaration_hover(&self, path: &Path, line: usize, column: usize) -> Option<String> {
        let text = self
            .index
            .hover(&self.source.offsets(Some(path), line, column).ok()?)?;
        Some(format!(
            "{text}\nDeclared syntax (unverified; no checked type is inferred)"
        ))
    }
    pub fn macro_info(&self, path: &Path, line: usize, column: usize) -> Option<(Label, String)> {
        let offsets = self.source.offsets(Some(path), line, column).ok()?;
        let text = self.index.hover(&offsets)?;
        if !text.starts_with("macro ") {
            return None;
        }
        Some((self.index.definition(&offsets)?.clone(), text.to_owned()))
    }
    pub fn macro_expansion(&self, path: &Path, line: usize, column: usize) -> serde_json::Value {
        let offsets = self
            .source
            .offsets(Some(path), line, column)
            .unwrap_or_default();
        let mut report = self.macros.clone();
        report
            .expansions
            .retain(|e| offsets.iter().any(|o| contains(e.call, *o)));
        report.located(&self.source)
    }
    pub fn completions(
        &self,
        path: &Path,
        line: usize,
        column: usize,
    ) -> Vec<(String, String, u32)> {
        self.source
            .offsets(Some(path), line, column)
            .map(|o| self.index.completions(path, &o))
            .unwrap_or_default()
    }
    pub fn unresolved(source: Source, error: Error) -> Result<Self, Error> {
        let syntax = crate::sexpr::editor_prefix(source.text()).map_err(|_| error.clone())?;
        let index = syntax_index::Index::new(&source, &syntax, true);
        Ok(Self {
            source,
            syntax,
            index,
            error: Some(error),
            checked: None,
            links: vec![],
            macros: Default::default(),
            performance_warnings: Default::default(),
        })
    }
}
