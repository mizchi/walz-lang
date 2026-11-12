//! Type equations at an ordinary call boundary; final checking is unchanged.
use super::*;
impl Checker<'_, '_> {
    pub(super) fn infer_call(
        &mut self,
        node: &Sexp,
        mut call: crate::generics::InferredCall<'_>,
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
        expected: Option<&Type>,
    ) -> Result<Expr, Error> {
        let callee = &node.list().unwrap()[1];
        let values = &node.list().unwrap()[2..];
        if values.len() != call.params.len() {
            return Err(call
                .error(
                    self.tables.source,
                    node,
                    "generic call argument count mismatch",
                )
                .with_code("E_CALL_ARITY"));
        }
        if let Some(ty) = expected.filter(|t| **t != Type::Never) {
            let syntax = self.inferred_syntax(node, ty)?.syntax;
            call.unify(&call.result.clone(), &syntax)
                .map_err(|m| call.error(self.tables.source, node, m))?;
        }
        let mut remaining = (0..values.len()).collect::<Vec<_>>();
        loop {
            // Once every type argument is known, let the ordinary concrete
            // signature supply hints, including newly materialized heap types.
            if call.is_complete() {
                break;
            }
            let mut progressed = false;
            let mut deferred = None;
            for i in remaining.clone() {
                let hint = call
                    .hint(&call.params[i])
                    .map_err(|m| call.error(self.tables.source, node, m))?
                    .and_then(|n| self.tables.ty(&n).ok());
                match self.probe(&values[i], scope, loops, hint.as_ref()) {
                    Ok(ty) => {
                        if ty != Type::Never {
                            let syntax = self.inferred_syntax(&values[i], &ty)?.syntax;
                            call.unify(&call.params[i].clone(), &syntax)
                                .map_err(|m| call.error(self.tables.source, &values[i], m))?;
                        }
                        remaining.retain(|n| *n != i);
                        progressed = true;
                    }
                    Err(e)
                        if matches!(
                            e.diagnostic().code,
                            "E_LITERAL_TYPE" | "E_GENERIC_INFERENCE"
                        ) =>
                    {
                        deferred = Some(e);
                    }
                    Err(e) => return Err(e),
                }
            }
            // Callable bounds provide type equations, not permission to
            // skip the existing callable shape/effect checks.
            for clause in call.callable.clone() {
                let p = clause.list().unwrap();
                if let Some(heap) = call
                    .hint(&p[1])
                    .map_err(|m| call.error(self.tables.source, node, m))?
                    && let Ok(id) = self.tables.type_id(&heap)
                    && let TypeKind::Closure { function, .. } = self.module.types[id.0].kind
                {
                    let f = &self.module.functions[function.0];
                    let params = &p[2].list().unwrap()[1..];
                    if params.len() == f.params.len() {
                        for (pattern, id) in params.iter().zip(&f.params) {
                            call.unify(
                                pattern,
                                &self.inferred_syntax(node, &f.locals[id.0].ty)?.syntax,
                            )
                            .map_err(|m| call.error(self.tables.source, node, m))?;
                        }
                        call.unify(
                            &p[3].list().unwrap()[1],
                            &self.inferred_syntax(node, &f.result)?.syntax,
                        )
                        .map_err(|m| call.error(self.tables.source, node, m))?;
                    }
                }
            }
            if remaining.is_empty() {
                break;
            }
            if !progressed {
                return Err(call
                    .error(
                        self.tables.source,
                        node,
                        "cannot uniquely infer argument types",
                    )
                    .note(
                        deferred
                            .map(|e| e.diagnostic().message.clone())
                            .unwrap_or_default(),
                    ));
            }
        }
        let concrete = call.finish(self.tables.source, callee, &self.function_name)?;
        let mut rewritten = node.clone();
        if let crate::sexpr::Kind::List(p) = &mut rewritten.kind {
            p[1].kind = crate::sexpr::Kind::Atom(concrete);
        }
        self.expr_expected(&rewritten, scope, loops, tail, expected)
    }
}
