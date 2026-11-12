//! Common lexical forms avoid the large folded-instruction dispatcher frame.
use super::*;

impl Checker<'_, '_> {
    pub(super) fn let_expr(
        &mut self,
        node: &Sexp,
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
        expected: Option<&Type>,
    ) -> Result<Expr, Error> {
        use ExprKind::*;
        let tables = self.tables;
        let items = tables.list(node)?;
        let (ty, kind) = {
            if !(4..=5).contains(&items.len()) {
                return Err(tables.error(node, "expected (let $name [type] initializer body)"));
            }
            if items[1].list().is_some() {
                if items[1].head() == Some("@fields") {
                    return self.struct_let(node, items, scope, loops, tail, expected);
                }
                return self.pair_let(node, items, scope, loops, tail, expected);
            }
            let name = self.tables.name(&items[1])?;
            let (annotation, start) = if items.len() == 5 {
                (Some(tables.ty(&items[2])?), 3)
            } else {
                (None, 2)
            };
            let value =
                self.expr_expected(&items[start], scope, loops, false, annotation.as_ref())?;
            if matches!(value.ty, Type::Unit | Type::Never) {
                return Err(self
                    .tables
                    .error(&items[start], "let initializer must produce a value"));
            }
            let binding_type = annotation.unwrap_or_else(|| value.ty.clone());
            self.check(&items[start], &binding_type, &value)?;
            let id = self.local(name.clone(), items[1].span, binding_type, BindingKind::Let);
            let mut nested = scope.clone();
            nested.insert(name, id);
            let body = self.expr_expected(&items[start + 1], &nested, loops, tail, expected)?;
            (
                body.ty.clone(),
                Let {
                    local: id,
                    value: Box::new(value),
                    body: Box::new(body),
                },
            )
        };
        Ok(Expr {
            ty,
            kind,
            span: node.span,
        })
    }
    pub(super) fn block_expr(
        &mut self,
        node: &Sexp,
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
        _expected: Option<&Type>,
    ) -> Result<Expr, Error> {
        use ExprKind::*;
        let tables = self.tables;
        let items = tables.list(node)?;
        let (ty, kind) = {
            let named = items
                .get(1)
                .and_then(Sexp::atom)
                .is_some_and(|n| n.starts_with('$'));
            let (result, start) = self.result(items, if named { 2 } else { 1 })?;
            let mut nested_labels = loops.to_vec();
            let label = if named {
                let id = LoopId(self.next_loop);
                self.next_loop += 1;
                nested_labels.push(LoopContext {
                    name: tables.name(&items[1])?,
                    id,
                    bindings: vec![],
                    block: Some(result.clone()),
                });
                Some(id)
            } else {
                None
            };
            let mut expressions = Vec::new();
            for (i, expression) in items[start..].iter().enumerate() {
                let last = i + start + 1 == items.len();
                let expression = self.expr_expected(
                    expression,
                    scope,
                    &nested_labels,
                    last && tail,
                    Some(if last { &result } else { &Type::Unit }),
                )?;
                if !last && self.is_owned(&expression.ty) {
                    return Err(tables.error(&items[start + i], "cannot discard an owned value"));
                }
                let expected = if last { &result } else { &Type::Unit };
                self.check(&items[start + i], expected, &expression)?;
                expressions.push(expression);
            }
            if expressions.is_empty() && result != Type::Unit {
                return Err(self.tables.error(node, "block result needs a value"));
            }
            let ty = if expressions.last().is_some_and(|e| e.ty == Type::Never) {
                Type::Never
            } else {
                result.clone()
            };
            if let Some(label) = label {
                (
                    result.clone(),
                    Label {
                        label,
                        body: Box::new(Expr {
                            ty: result,
                            span: node.span,
                            kind: Block(expressions),
                        }),
                    },
                )
            } else {
                (ty, Block(expressions))
            }
        };
        Ok(Expr {
            ty,
            kind,
            span: node.span,
        })
    }
    pub(super) fn if_expr(
        &mut self,
        node: &Sexp,
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
        _expected: Option<&Type>,
    ) -> Result<Expr, Error> {
        use ExprKind::*;
        let tables = self.tables;
        let items = tables.list(node)?;
        let count = |n| tables.count(node, items, n);
        let (ty, kind) = {
            let (result, start) = self.result(items, 1)?;
            count(start + 3)?;
            let condition =
                self.expr_as(&items[start], scope, loops, false, &Type::Num(NumType::I32))?;
            let then_node = self.tables.branch(&items[start + 1], "then")?;
            let else_node = self.tables.branch(&items[start + 2], "else")?;
            let then_body = self.expr_as(then_node, scope, loops, tail, &result)?;
            let else_body = self.expr_as(else_node, scope, loops, tail, &result)?;
            (
                result,
                If {
                    condition: Box::new(condition),
                    then_body: Box::new(then_body),
                    else_body: Box::new(else_body),
                },
            )
        };
        Ok(Expr {
            ty,
            kind,
            span: node.span,
        })
    }
}
