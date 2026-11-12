use super::*;

impl Checker<'_, '_> {
    pub(super) fn type_info(&self, ty: &Type) -> crate::macros::TypeInfo {
        use crate::macros::TypeInfo as Info;
        let boxed = |ty: &Type| Box::new(self.type_info(ty));
        let name = |id: &TypeId| self.module.types[id.0].name.clone();
        match ty {
            Type::Num(n) => Info::Scalar {
                name: n.wat().into(),
            },
            Type::Unit => Info::Unit,
            Type::Newtype(id) => Info::Nominal { name: name(id) },
            Type::Ref { ty, nullable } => Info::Ref {
                name: name(ty),
                nullable: *nullable,
            },
            Type::Resource(id) => Info::Resource { name: name(id) },
            Type::Borrow(id) => Info::Borrow { name: name(id) },
            Type::Value(_) => match self.tables.value(ty).expect("value type") {
                ValueType::Bytes => Info::Bytes,
                ValueType::Text => Info::String,
                ValueType::List(t) => Info::List { element: boxed(&t) },
                ValueType::Option(t) => Info::Option { element: boxed(&t) },
                ValueType::Own(t) => Info::Box { element: boxed(&t) },
                ValueType::Future(t) => Info::Future { element: boxed(&t) },
                ValueType::Stream(t) => Info::Stream { element: boxed(&t) },
                ValueType::Task(t) => Info::Task { element: boxed(&t) },
                ValueType::Pair { first, second } => Info::Tuple {
                    first: boxed(&first),
                    second: boxed(&second),
                },
                ValueType::Result { ok, err } => Info::Result {
                    ok: boxed(&ok),
                    err: boxed(&err),
                },
            },
            Type::Never => unreachable!("bottom is not reified"),
        }
    }
    pub(super) fn project_type(
        &self,
        node: &Sexp,
        ty: &Type,
        projection: crate::macros::Projection,
    ) -> Result<crate::macros::Inferred, Error> {
        use crate::macros::Projection as P;
        if projection == P::Identity {
            return self.inferred_syntax(node, ty);
        }
        let payload = match (projection, self.tables.value(ty)) {
            (P::OptionElement, Some(ValueType::Option(t)))
            | (P::ListElement, Some(ValueType::List(t)))
            | (P::BoxElement, Some(ValueType::Own(t)))
            | (P::FutureElement, Some(ValueType::Future(t)))
            | (P::StreamElement, Some(ValueType::Stream(t)))
            | (P::TaskElement, Some(ValueType::Task(t))) => Some(t),
            (P::ResultOk, Some(ValueType::Result { ok, .. })) => Some(ok),
            (P::ResultErr, Some(ValueType::Result { err, .. })) => Some(err),
            (P::TupleFirst, Some(ValueType::Pair { first, .. })) => Some(first),
            (P::TupleSecond, Some(ValueType::Pair { second, .. })) => Some(second),
            _ => None,
        };
        if let Some(t) = payload {
            return self.inferred_syntax(node, &t);
        }
        if let Type::Ref { ty: id, .. } = ty {
            let decl = &self.module.types[id.0];
            if projection == P::RefHeap {
                return Ok(crate::macros::Inferred {
                    syntax: Sexp {
                        kind: crate::sexpr::Kind::Atom(decl.name.clone()),
                        span: node.span,
                        context: Default::default(),
                    },
                    info: crate::macros::TypeInfo::Nominal {
                        name: decl.name.clone(),
                    },
                    classes: Default::default(),
                });
            }
            if projection == P::ArrayElement
                && let TypeKind::Array(field) = &decl.kind
            {
                if let Some((modules, origins)) = self.tables.privacy {
                    modules.validate_representation(
                        self.tables.source,
                        &decl.name,
                        node.span,
                        origins,
                    )?;
                }
                return self.inferred_syntax(node, &field.ty);
            }
        }
        Err(self
            .tables
            .error(
                node,
                format!(
                    "type query {} does not apply to {}",
                    projection.name(),
                    self.tables.type_name(ty)
                ),
            )
            .with_code("E_MACRO_TYPE"))
    }
}
