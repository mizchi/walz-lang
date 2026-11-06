//! Portable interface shapes. This emits a WIT contract, not an ABI adapter.

use crate::Error;
use crate::ir::*;
use std::collections::BTreeSet;

fn identifier(name: &str) -> bool {
    name.split('-').all(|word| {
        word.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
            && word
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    })
}

struct Types<'a> {
    module: &'a Module,
    unit_name: String,
    needs_unit: bool,
}

pub(crate) fn validate_type(module: &Module, ty: &Type) -> Result<(), Error> {
    Types {
        module,
        unit_name: "wlt-unit".into(),
        needs_unit: false,
    }
    .render(ty)
    .map(|_| ())
}

impl Types<'_> {
    fn render(&mut self, ty: &Type) -> Result<String, Error> {
        Ok(match ty {
            Type::Newtype(id) => {
                let TypeKind::Newtype(n) = self.module.types[id.0].kind else {
                    unreachable!()
                };
                self.render(&Type::Num(n))?
            }
            Type::Num(NumType::I32) => "s32".into(),
            Type::Num(NumType::I64) => "s64".into(),
            Type::Num(NumType::F32) => "f32".into(),
            Type::Num(NumType::F64) => "f64".into(),
            Type::Num(NumType::V128) => {
                return Err(Error::message(
                    "v128 has no WIT/JavaScript value representation; use scalar lanes or bytes at the boundary instead".into(),
                ).with_code("E_ABI_TYPE"));
            }
            Type::Unit => {
                self.needs_unit = true;
                format!("%{}", self.unit_name)
            }
            Type::Value(id) => match &self.module.types[id.0].kind {
                TypeKind::Value(ValueType::Future(payload) | ValueType::Stream(payload)) => {
                    let name = if matches!(
                        self.module.types[id.0].kind,
                        TypeKind::Value(ValueType::Future(_))
                    ) {
                        "future"
                    } else {
                        "stream"
                    };
                    if *payload == Type::Unit {
                        name.into()
                    } else {
                        format!("{name}<{}>", self.render(payload)?)
                    }
                }
                TypeKind::Value(ValueType::List(payload)) => {
                    format!("list<{}>", self.render(payload)?)
                }
                TypeKind::Value(ValueType::Option(payload)) => {
                    format!("option<{}>", self.render(payload)?)
                }
                TypeKind::Value(ValueType::Result { ok, err }) => match (ok, err) {
                    (Type::Unit, Type::Unit) => "result".into(),
                    (Type::Unit, err) => format!("result<_, {}>", self.render(err)?),
                    (ok, Type::Unit) => format!("result<{}>", self.render(ok)?),
                    (ok, err) => format!("result<{}, {}>", self.render(ok)?, self.render(err)?),
                },
                TypeKind::Value(ValueType::Pair { first, second }) => {
                    format!("tuple<{}, {}>", self.render(first)?, self.render(second)?)
                }
                TypeKind::Value(ValueType::Bytes) => "list<u8>".into(),
                TypeKind::Value(ValueType::Text) => "string".into(),
                TypeKind::Value(ValueType::Own(_)) => {
                    return Err(Error::message(
                        "owned boxes require an explicit resource adapter for WIT".into(),
                    ));
                }
                _ => unreachable!("checked value type"),
            },
            Type::Resource(_) | Type::Borrow(_) => {
                return Err(Error::message(
                    "resources require a host resource adapter and cannot cross value exports"
                        .into(),
                ));
            }
            Type::Ref { .. } => return Err(Error::message(
                "GC references are not WIT value types; use an explicit value or resource adapter"
                    .into(),
            )),
            Type::Never => return Err(Error::message("never is not a WIT value type".into())),
        })
    }
}

/// Export representable public functions of a checked module as a WIT package.
/// Unknown/escaping exceptions and nonportable types require explicit adapters.
pub fn emit(module: &Module) -> Result<String, Error> {
    let lowered = crate::newtypes::lower(module);
    let module = &lowered;
    let exported = module
        .functions
        .iter()
        .enumerate()
        .filter(|(index, function)| {
            !function.exports.is_empty()
                && !module.tests.iter().any(|test| test.function.0 == *index)
        })
        .collect::<Vec<_>>();
    if exported.is_empty() {
        return Err(Error::message("no exported functions for WIT".into()));
    }
    let names = exported
        .iter()
        .flat_map(|(_, f)| f.exports.iter())
        .collect::<BTreeSet<_>>();
    let mut unit_name = String::from("wlt-unit");
    let mut suffix = 0;
    while names.contains(&unit_name) {
        suffix += 1;
        unit_name = format!("wlt-unit-a{suffix}");
    }
    let mut types = Types {
        module,
        unit_name,
        needs_unit: false,
    };
    let inferred = crate::effects::infer(module);
    let mut declarations = String::new();
    for (index, function) in exported {
        let effects = &inferred[index];
        if effects.throws_unknown || !effects.throws.is_empty() {
            return Err(Error::message(format!(
                "WIT export `{}` has unhandled exceptions; convert them explicitly to Result",
                function.name
            )));
        }
        let params = function
            .params
            .iter()
            .enumerate()
            .map(|(i, id)| {
                Ok(format!(
                    "p{i}: {}",
                    types
                        .render(&function.locals[id.0].ty)
                        .map_err(|error| Error::message(format!(
                            "WIT export `{}` parameter {i}: {error}",
                            function.name
                        )))?
                ))
            })
            .collect::<Result<Vec<_>, Error>>()?
            .join(", ");
        let result = if function.result == Type::Unit {
            String::new()
        } else {
            format!(
                " -> {}",
                types
                    .render(&function.result)
                    .map_err(|error| Error::message(format!(
                        "WIT export `{}` result: {error}",
                        function.name
                    )))?
            )
        };
        for name in &function.exports {
            if !identifier(name) {
                return Err(Error::message(format!(
                    "invalid WIT export name `{name}`; use lowercase kebab-case"
                )));
            }
            declarations.push_str(&format!(
                "  %{name}: {}func({params}){result};\n",
                if effects.suspends { "async " } else { "" }
            ));
        }
    }
    let unit = if types.needs_unit {
        format!("  enum %{} {{ value }}\n\n", types.unit_name)
    } else {
        String::new()
    };
    Ok(format!(
        "package wlt:generated@0.1.0;\n\ninterface api {{\n{unit}{declarations}}}\n\nworld program {{\n  export api;\n}}\n"
    ))
}
