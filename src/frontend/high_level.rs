//! Surface conveniences lower to the existing checked IR. No runtime operation
//! or new ownership/effect rule is introduced here.
use super::*;

fn expression(span: Span, ty: Type, kind: ExprKind) -> Expr {
    Expr { span, ty, kind }
}
fn generated(node: &Sexp) -> Span {
    Span {
        start: node.span.start,
        end: node.span.start,
    }
}
fn let_in(span: Span, local: LocalId, value: Expr, body: Expr) -> Expr {
    expression(
        span,
        body.ty.clone(),
        ExprKind::Let {
            local,
            value: Box::new(value),
            body: Box::new(body),
        },
    )
}

impl Checker<'_, '_> {
    // Value types can be interned during expression checking, after module's
    // type table was built. Nominal storage still uses the common shared check.
    fn shared_data(&self, ty: &Type) -> bool {
        if self.is_owned(ty) || matches!(ty, Type::Borrow(_)) {
            return false;
        }
        match self.tables.value(ty) {
            Some(ValueType::Bytes | ValueType::Text) => true,
            Some(ValueType::List(t) | ValueType::Option(t)) => self.shared_data(&t),
            Some(ValueType::Pair { first, second }) => {
                self.shared_data(&first) && self.shared_data(&second)
            }
            Some(ValueType::Result { ok, err }) => self.shared_data(&ok) && self.shared_data(&err),
            Some(_) => false,
            None => {
                crate::bounds::shared(self.module, &crate::ownership::Kinds::new(self.module), ty)
            }
        }
    }

    /// LocalId temporaries never enter the source Scope. Their zero-width span
    /// is not a source declaration; display names use the reserved prefix.
    fn temporary(&mut self, node: &Sexp, ty: Type, kind: BindingKind) -> LocalId {
        self.local(
            format!("$__high.{}", self.locals.len()),
            generated(node),
            ty,
            kind,
        )
    }

    fn read_local(&self, local: LocalId, span: Span) -> Expr {
        expression(
            span,
            self.locals[local.0].ty.clone(),
            ExprKind::Local(local),
        )
    }

    pub(super) fn struct_let(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
        expected: Option<&Type>,
    ) -> Result<Expr, Error> {
        self.tables.count(node, items, 4)?;
        let subject = self.expr(&items[2], scope, loops, false)?;
        let mut result = self.field_pattern(
            (&items[1], subject),
            &items[3],
            scope,
            loops,
            tail,
            expected,
        )?;
        result.span = node.span;
        Ok(result)
    }

    fn field_pattern(
        &mut self,
        binding: (&Sexp, Expr),
        body: &Sexp,
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
        expected: Option<&Type>,
    ) -> Result<Expr, Error> {
        let (pattern, subject) = binding;
        let fail = |node: &Sexp, message| {
            self.tables
                .error(node, message)
                .with_code("E_STRUCT_PATTERN")
        };
        let Type::Ref {
            ty,
            nullable: false,
        } = subject.ty
        else {
            return Err(fail(
                pattern,
                "field pattern requires a non-null struct reference",
            ));
        };
        if !matches!(self.module.types[ty.0].kind, TypeKind::Struct(_)) {
            return Err(fail(pattern, "field pattern requires a struct reference"));
        }
        if !self.shared_data(&subject.ty) {
            return Err(fail(
                pattern,
                "field patterns require shared data without owned values or borrows",
            ));
        }
        let mut fields = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut plan = Vec::new();
        for binding in &pattern.list().unwrap()[1..] {
            let Some([field, name]) = binding.list() else {
                return Err(fail(binding, "expected :field $binding in field pattern"));
            };
            let (index, declaration) = self.field(pattern, ty, field)?;
            if !fields.insert(index) {
                return Err(fail(field, "duplicate field in field pattern"));
            }
            let name_text = self.tables.name(name)?;
            if !names.insert(name_text.clone()) {
                return Err(fail(name, "duplicate field pattern binding"));
            }
            plan.push((index, declaration.ty, name, name_text));
        }
        let target = self.temporary(pattern, subject.ty.clone(), BindingKind::Let);
        let mut nested = scope.clone();
        let mut bindings = Vec::new();
        for (field, ty_field, name, name_text) in plan {
            let local = self.local(
                name_text.clone(),
                name.span,
                ty_field.clone(),
                BindingKind::Let,
            );
            nested.insert(name_text, local);
            let value = expression(
                name.span,
                ty_field,
                ExprKind::StructGet {
                    ty,
                    field,
                    object: Box::new(self.read_local(target, pattern.span)),
                },
            );
            bindings.push((local, value));
        }
        // Require an access to the representation, so even an unused binding
        // reaches the normal module privacy validator before optimization.
        if bindings.is_empty() {
            return Err(self
                .tables
                .error(pattern, "field pattern requires at least one field")
                .with_code("E_STRUCT_PATTERN"));
        }
        let span = Span {
            start: pattern.span.start,
            end: body.span.end,
        };
        let mut body = self.expr_expected(body, &nested, loops, tail, expected)?;
        for (local, value) in bindings.into_iter().rev() {
            body = let_in(span, local, value, body);
        }
        Ok(let_in(span, target, subject, body))
    }

    pub(super) fn struct_update(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
    ) -> Result<(Type, ExprKind), Error> {
        let fail = |n: &Sexp, message| self.tables.error(n, message).with_code("E_STRUCT_UPDATE");
        if items.len() != 4 {
            return Err(fail(
                node,
                "expected (%struct.update $Type object ($field $old new-value))",
            ));
        }
        let Some([field, name, update]) = items[3].list() else {
            return Err(fail(&items[3], "expected ($field $old new-value)"));
        };
        let ty = self.tables.type_id(&items[1])?;
        let (field, declaration) = self.field(node, ty, field)?;
        if !declaration.mutable {
            return Err(fail(node, "cannot update immutable field"));
        }
        if declaration.ty == Type::Unit || !self.shared_data(&declaration.ty) {
            return Err(fail(node, "struct.update requires a shared non-unit field"));
        }
        let target_type = Type::Ref { ty, nullable: true };
        let value = self.expr_as(&items[2], scope, loops, false, &target_type)?;
        if !self.shared_data(&value.ty) {
            return Err(fail(&items[2], "struct.update requires shared data"));
        }
        let target = self.temporary(
            node,
            if value.ty == Type::Never {
                target_type
            } else {
                value.ty.clone()
            },
            BindingKind::Let,
        );
        let name_text = self.tables.name(name)?;
        let old = self.local(
            name_text.clone(),
            name.span,
            declaration.ty.clone(),
            BindingKind::Let,
        );
        let mut nested = scope.clone();
        nested.insert(name_text, old);
        let updated = self.expr_as(update, &nested, loops, false, &declaration.ty)?;
        let get = expression(
            name.span,
            declaration.ty,
            ExprKind::StructGet {
                ty,
                field,
                object: Box::new(self.read_local(target, generated(node))),
            },
        );
        let set = expression(
            node.span,
            Type::Unit,
            ExprKind::StructSet {
                ty,
                field,
                object: Box::new(self.read_local(target, generated(node))),
                value: Box::new(updated),
            },
        );
        let body = let_in(node.span, target, value, let_in(node.span, old, get, set));
        Ok((body.ty, body.kind))
    }

    pub(super) fn for_each(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
    ) -> Result<(Type, ExprKind), Error> {
        self.tables.count(node, items, 4)?;
        let subject = self.expr(&items[2], scope, loops, false)?;
        let Some(ValueType::List(payload)) = self.tables.value(&subject.ty) else {
            return Err(self
                .tables
                .error(&items[2], "for-each requires a list value")
                .with_code("E_FOR_EACH"));
        };
        if payload == Type::Unit || !self.shared_data(&payload) {
            return Err(self
                .tables
                .error(&items[2], "for-each requires shared non-unit list elements")
                .with_code("E_FOR_EACH"));
        }
        let Type::Value(list) = subject.ty else {
            unreachable!()
        };
        let i32ty = Type::Num(NumType::I32);
        let label = LoopId(self.next_loop);
        self.next_loop += 1;
        let source = self.temporary(node, subject.ty.clone(), BindingKind::Let);
        let length = self.temporary(node, i32ty.clone(), BindingKind::Let);
        let index = self.temporary(node, i32ty.clone(), BindingKind::Loop(label));
        let mut nested_loops = loops.to_vec();
        nested_loops.push(LoopContext {
            name: String::new(),
            id: label,
            bindings: vec![index],
            block: None,
        });
        let element = expression(
            items[1].span,
            payload.clone(),
            ExprKind::List {
                op: ListOp::Get,
                list,
                args: vec![
                    self.read_local(source, node.span),
                    self.read_local(index, node.span),
                ],
            },
        );
        let body = if items[1].head() == Some("@fields") {
            self.field_pattern(
                (&items[1], element),
                &items[3],
                scope,
                &nested_loops,
                false,
                Some(&Type::Unit),
            )?
        } else if items[1].list().is_some() {
            let Some(shape @ ValueType::Pair { .. }) = self.tables.value(&payload) else {
                return Err(self
                    .tables
                    .error(&items[1], "for-each tuple pattern requires a pair element"));
            };
            let Type::Value(ty) = payload else {
                unreachable!()
            };
            let (nested, bindings) =
                self.payload_bindings(&items[1], &shape.variants(ty)[0].payloads, scope)?;
            let body = self.expr_as(&items[3], &nested, &nested_loops, false, &Type::Unit)?;
            expression(
                node.span,
                Type::Unit,
                ExprKind::Match {
                    ty,
                    subject: Box::new(element),
                    arms: vec![MatchArm {
                        variant: 0,
                        bindings,
                        body,
                    }],
                },
            )
        } else {
            let name = self.tables.name(&items[1])?;
            let local = self.local(name.clone(), items[1].span, payload, BindingKind::Let);
            let mut nested = scope.clone();
            nested.insert(name, local);
            let body = self.expr_as(&items[3], &nested, &nested_loops, false, &Type::Unit)?;
            let_in(node.span, local, element, body)
        };
        self.check(&items[3], &Type::Unit, &body)?;
        let synthetic = generated(node);
        let zero = || {
            expression(
                synthetic,
                i32ty.clone(),
                ExprKind::Const {
                    ty: NumType::I32,
                    literal: "0".into(),
                },
            )
        };
        let condition = expression(
            synthetic,
            i32ty.clone(),
            ExprKind::Numeric {
                op: NumericOp::parse("i32.lt_u").unwrap(),
                args: vec![
                    self.read_local(index, synthetic),
                    self.read_local(length, synthetic),
                ],
            },
        );
        let next = expression(
            synthetic,
            i32ty.clone(),
            ExprKind::Numeric {
                op: NumericOp::parse("i32.add").unwrap(),
                args: vec![
                    self.read_local(index, synthetic),
                    expression(
                        synthetic,
                        i32ty.clone(),
                        ExprKind::Const {
                            ty: NumType::I32,
                            literal: "1".into(),
                        },
                    ),
                ],
            },
        );
        let again = expression(
            synthetic,
            Type::Never,
            ExprKind::Recur {
                label,
                assignments: vec![(index, next)],
            },
        );
        let body = expression(
            node.span,
            Type::Unit,
            ExprKind::If {
                condition: Box::new(condition),
                then_body: Box::new(expression(
                    node.span,
                    Type::Never,
                    ExprKind::Block(vec![body, again]),
                )),
                else_body: Box::new(expression(synthetic, Type::Unit, ExprKind::Nop)),
            },
        );
        let loop_expr = expression(
            node.span,
            Type::Unit,
            ExprKind::Loop {
                label,
                bindings: vec![(index, zero())],
                body: Box::new(body),
            },
        );
        let len = expression(
            synthetic,
            i32ty,
            ExprKind::List {
                op: ListOp::Len,
                list,
                args: vec![self.read_local(source, synthetic)],
            },
        );
        let body = let_in(
            node.span,
            source,
            subject,
            let_in(node.span, length, len, loop_expr),
        );
        Ok((body.ty, body.kind))
    }
}
