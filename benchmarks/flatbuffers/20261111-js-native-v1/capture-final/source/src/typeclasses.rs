//! Sealed scalar type classes. Instances resolve to existing Wasm operations;
//! neither dictionaries nor new runtime representations reach the checked IR.
use crate::ir::{Expr, ExprKind, NumType, NumericOp, Span, Type};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Class {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ord,
    Logical,
    Bitwise,
    Shift,
}

impl Class {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "Add" => Self::Add,
            "Sub" => Self::Sub,
            "Mul" => Self::Mul,
            "Div" => Self::Div,
            "Eq" => Self::Eq,
            "Ord" => Self::Ord,
            "Logical" => Self::Logical,
            "Bitwise" => Self::Bitwise,
            "Shift" => Self::Shift,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Add => "Add",
            Self::Sub => "Sub",
            Self::Mul => "Mul",
            Self::Div => "Div",
            Self::Eq => "Eq",
            Self::Ord => "Ord",
            Self::Logical => "Logical",
            Self::Bitwise => "Bitwise",
            Self::Shift => "Shift",
        }
    }

    pub fn supports(self, ty: &Type) -> bool {
        matches!(
            (self, ty),
            (
                Self::Add | Self::Sub | Self::Mul | Self::Div,
                Type::Num(NumType::I32 | NumType::I64 | NumType::F32 | NumType::F64)
            ) | (
                Self::Eq | Self::Ord | Self::Bitwise | Self::Shift,
                Type::Num(NumType::I32 | NumType::I64)
            ) | (Self::Logical, Type::Num(NumType::I32))
        )
    }

    pub fn supported_types(self) -> &'static str {
        match self {
            Self::Add | Self::Sub | Self::Mul | Self::Div => "i32/i64/f32/f64",
            Self::Eq | Self::Ord | Self::Bitwise | Self::Shift => "i32/i64",
            Self::Logical => "i32",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Operator {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    LogicalAnd,
    LogicalOr,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    ShrU,
}

impl Operator {
    pub fn parse(symbol: &str) -> Option<Self> {
        Some(match symbol {
            "+" => Self::Add,
            "-" => Self::Sub,
            "*" => Self::Mul,
            "/" => Self::Div,
            "=" => Self::Eq,
            "!=" => Self::Ne,
            "<" => Self::Lt,
            "<=" => Self::Le,
            ">" => Self::Gt,
            ">=" => Self::Ge,
            "&&" => Self::LogicalAnd,
            "||" => Self::LogicalOr,
            "&" => Self::BitAnd,
            "|" => Self::BitOr,
            "^" => Self::BitXor,
            "<<" => Self::Shl,
            ">>" => Self::Shr,
            ">>>" => Self::ShrU,
            _ => return None,
        })
    }

    pub fn class(self) -> Class {
        match self {
            Self::Add => Class::Add,
            Self::Sub => Class::Sub,
            Self::Mul => Class::Mul,
            Self::Div => Class::Div,
            Self::Eq | Self::Ne => Class::Eq,
            Self::Lt | Self::Le | Self::Gt | Self::Ge => Class::Ord,
            Self::LogicalAnd | Self::LogicalOr => Class::Logical,
            Self::BitAnd | Self::BitOr | Self::BitXor => Class::Bitwise,
            Self::Shl | Self::Shr | Self::ShrU => Class::Shift,
        }
    }

    pub fn preserves_type(self) -> bool {
        matches!(
            self,
            Self::Add
                | Self::Sub
                | Self::Mul
                | Self::Div
                | Self::BitAnd
                | Self::BitOr
                | Self::BitXor
                | Self::Shl
                | Self::Shr
                | Self::ShrU
        )
    }

    pub fn fixed_input(self) -> Option<Type> {
        matches!(self, Self::LogicalAnd | Self::LogicalOr).then_some(Type::Num(NumType::I32))
    }

    pub fn resolve(self, input: &Type) -> Option<Resolved> {
        if !self.class().supports(input) {
            return None;
        }
        if matches!(self, Self::LogicalAnd) {
            return Some(Resolved::LogicalAnd);
        }
        if matches!(self, Self::LogicalOr) {
            return Some(Resolved::LogicalOr);
        }
        let Type::Num(ty) = input else {
            return None;
        };
        let method = match self {
            Self::Add => "add",
            Self::Sub => "sub",
            Self::Mul => "mul",
            Self::Div if matches!(ty, NumType::I32 | NumType::I64) => "div_s",
            Self::Div => "div",
            Self::Eq => "eq",
            Self::Ne => "ne",
            Self::Lt => "lt_s",
            Self::Le => "le_s",
            Self::Gt => "gt_s",
            Self::Ge => "ge_s",
            Self::BitAnd => "and",
            Self::BitOr => "or",
            Self::BitXor => "xor",
            Self::Shl => "shl",
            Self::Shr => "shr_s",
            Self::ShrU => "shr_u",
            Self::LogicalAnd | Self::LogicalOr => unreachable!(),
        };
        NumericOp::parse(&format!("{}.{method}", ty.wat())).map(Resolved::Numeric)
    }
}

/// Logical operators are control flow. An eager numeric operation would execute
/// their RHS even when its value is not needed.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Resolved {
    Numeric(NumericOp),
    LogicalAnd,
    LogicalOr,
}

impl Resolved {
    pub fn output(self) -> NumType {
        match self {
            Self::Numeric(op) => op.signature().output,
            Self::LogicalAnd | Self::LogicalOr => NumType::I32,
        }
    }

    pub fn lower(self, [left, right]: [Expr; 2], span: Span) -> Expr {
        let kind = match self {
            Self::Numeric(op) => ExprKind::Numeric {
                op,
                args: vec![left, right],
            },
            Self::LogicalAnd | Self::LogicalOr => {
                let constant = |literal: &str| Expr {
                    ty: Type::Num(NumType::I32),
                    span: Span {
                        start: span.start,
                        end: span.start,
                    },
                    kind: ExprKind::Const {
                        ty: NumType::I32,
                        literal: literal.into(),
                    },
                };
                let truth = Expr {
                    ty: Type::Num(NumType::I32),
                    span: right.span,
                    kind: ExprKind::Numeric {
                        op: NumericOp::I32Ne,
                        args: vec![right, constant("0")],
                    },
                };
                let (then_body, else_body) = match self {
                    Self::LogicalAnd => (truth, constant("0")),
                    Self::LogicalOr => (constant("1"), truth),
                    Self::Numeric(_) => unreachable!(),
                };
                ExprKind::If {
                    condition: Box::new(left),
                    then_body: Box::new(then_body),
                    else_body: Box::new(else_body),
                }
            }
        };
        Expr {
            ty: Type::Num(self.output()),
            span,
            kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_method_has_the_declared_scalar_signature() {
        for symbol in [
            "+", "-", "*", "/", "=", "!=", "<", "<=", ">", ">=", "&&", "||", "&", "|", "^", "<<",
            ">>", ">>>",
        ] {
            let op = Operator::parse(symbol).unwrap();
            for ty in [
                NumType::I32,
                NumType::I64,
                NumType::F32,
                NumType::F64,
                NumType::V128,
            ] {
                let resolved = op.resolve(&Type::Num(ty));
                assert_eq!(resolved.is_some(), op.class().supports(&Type::Num(ty)));
                if let Some(resolved) = resolved {
                    assert_eq!(
                        resolved.output(),
                        if op.preserves_type() {
                            ty
                        } else {
                            NumType::I32
                        }
                    );
                    match resolved {
                        Resolved::Numeric(instruction) => {
                            let signature = instruction.signature();
                            assert_eq!(signature.arity, 2);
                            assert_eq!(signature.input, ty);
                            assert_eq!(
                                signature.may_trap,
                                matches!(op, Operator::Div)
                                    && matches!(ty, NumType::I32 | NumType::I64)
                            );
                        }
                        Resolved::LogicalAnd | Resolved::LogicalOr => {
                            assert_eq!(ty, NumType::I32);
                            assert_eq!(op.fixed_input(), Some(Type::Num(NumType::I32)));
                        }
                    }
                }
                if Class::Ord.supports(&Type::Num(ty)) {
                    assert!(Class::Eq.supports(&Type::Num(ty)));
                }
            }
        }
    }
}
