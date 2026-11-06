//! Read-only source queries over the same checked IR used by compilation.
use crate::{
    Error, bindings::BindingPolicy, diagnostic::Label, ir::*, sexpr::Sexp, source::Source,
};
use serde::Serialize;
use std::{collections::BTreeMap, path::Path};

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
    generics: &crate::generics::Expanded,
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
            let specialization = generics
                .instances
                .iter()
                .find(|(_, _, _, name)| name == &function.name)
                .and_then(|(_, _, display, _)| {
                    display
                        .find('<')
                        .map(|i| format!("{function_name}{}", &display[i..]))
                });
            let instantiations = generics
                .instances
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
        query(loaded, &syntax, &offsets, module, generics)
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
