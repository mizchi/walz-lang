//! The checked, closed expression language used by analysis and code generation.

use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

macro_rules! ids {
    ($($name:ident),*) => { $(
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
        pub struct $name(pub usize);
    )* };
}
ids!(TypeId, FunctionId, LocalId, LoopId, TagId);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NumType {
    I32,
    I64,
    F32,
    F64,
}

impl NumType {
    pub fn wat(self) -> &'static str {
        match self {
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "i32" => Self::I32,
            "i64" => Self::I64,
            "f32" => Self::F32,
            "f64" => Self::F64,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Type {
    Num(NumType),
    Ref {
        ty: TypeId,
        nullable: bool,
    },
    /// A semantic value whose hidden GC representation is not a source reference.
    Value(TypeId),
    Unit,
    Never,
}

impl Type {
    pub fn accepts(&self, actual: &Self) -> bool {
        actual == &Self::Never
            || self == actual
            || matches!((self, actual),
            (Self::Ref { ty: a, nullable: true }, Self::Ref { ty: b, .. }) if a == b)
    }

    pub(crate) fn format_with(&self, name: impl FnOnce(TypeId) -> String) -> String {
        match self {
            Self::Num(ty) => ty.wat().to_owned(),
            Self::Ref { ty, nullable } => format!(
                "(ref {}{})",
                if *nullable { "null " } else { "" },
                name(*ty)
            ),
            Self::Value(ty) => name(*ty),
            Self::Unit => "unit".into(),
            Self::Never => "never".into(),
        }
    }
}

/// The type and effect contract shared by checking, analysis, and emission.
#[derive(Clone, Copy, Debug)]
pub struct NumericSignature {
    pub name: &'static str,
    pub input: NumType,
    pub output: NumType,
    pub arity: usize,
    pub may_trap: bool,
}

macro_rules! numeric_ops {
    ($($variant:ident => ($name:literal, $input:ident, $output:ident, $arity:literal, $trap:literal)),* $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
        pub enum NumericOp { $($variant),* }
        impl NumericOp {
            pub fn parse(name: &str) -> Option<Self> {
                Some(match name { $($name => Self::$variant,)* _ => return None })
            }
            pub fn signature(self) -> NumericSignature {
                match self { $(Self::$variant => NumericSignature {
                    name: $name, input: NumType::$input, output: NumType::$output,
                    arity: $arity, may_trap: $trap,
                },)* }
            }
        }
    };
}

numeric_ops! {
    I32Add => ("i32.add", I32, I32, 2, false), I64Add => ("i64.add", I64, I64, 2, false),
    I32Sub => ("i32.sub", I32, I32, 2, false), I64Sub => ("i64.sub", I64, I64, 2, false),
    I32Mul => ("i32.mul", I32, I32, 2, false), I64Mul => ("i64.mul", I64, I64, 2, false),
    I32DivS => ("i32.div_s", I32, I32, 2, true), I64DivS => ("i64.div_s", I64, I64, 2, true),
    I32DivU => ("i32.div_u", I32, I32, 2, true), I64DivU => ("i64.div_u", I64, I64, 2, true),
    I32RemS => ("i32.rem_s", I32, I32, 2, true), I64RemS => ("i64.rem_s", I64, I64, 2, true),
    I32RemU => ("i32.rem_u", I32, I32, 2, true), I64RemU => ("i64.rem_u", I64, I64, 2, true),
    I32And => ("i32.and", I32, I32, 2, false), I64And => ("i64.and", I64, I64, 2, false),
    I32Or => ("i32.or", I32, I32, 2, false), I64Or => ("i64.or", I64, I64, 2, false),
    I32Xor => ("i32.xor", I32, I32, 2, false), I64Xor => ("i64.xor", I64, I64, 2, false),
    I32Shl => ("i32.shl", I32, I32, 2, false), I64Shl => ("i64.shl", I64, I64, 2, false),
    I32ShrS => ("i32.shr_s", I32, I32, 2, false), I64ShrS => ("i64.shr_s", I64, I64, 2, false),
    I32ShrU => ("i32.shr_u", I32, I32, 2, false), I64ShrU => ("i64.shr_u", I64, I64, 2, false),
    I32Eqz => ("i32.eqz", I32, I32, 1, false), I64Eqz => ("i64.eqz", I64, I32, 1, false),
    I32Eq => ("i32.eq", I32, I32, 2, false), I64Eq => ("i64.eq", I64, I32, 2, false),
    I32Ne => ("i32.ne", I32, I32, 2, false), I64Ne => ("i64.ne", I64, I32, 2, false),
    I32LtS => ("i32.lt_s", I32, I32, 2, false), I64LtS => ("i64.lt_s", I64, I32, 2, false),
    I32LeS => ("i32.le_s", I32, I32, 2, false), I64LeS => ("i64.le_s", I64, I32, 2, false),
    I32GtS => ("i32.gt_s", I32, I32, 2, false), I64GtS => ("i64.gt_s", I64, I32, 2, false),
    I32GeS => ("i32.ge_s", I32, I32, 2, false), I64GeS => ("i64.ge_s", I64, I32, 2, false),
    I32LtU => ("i32.lt_u", I32, I32, 2, false), I64LtU => ("i64.lt_u", I64, I32, 2, false),
    I32LeU => ("i32.le_u", I32, I32, 2, false), I64LeU => ("i64.le_u", I64, I32, 2, false),
    I32GtU => ("i32.gt_u", I32, I32, 2, false), I64GtU => ("i64.gt_u", I64, I32, 2, false),
    I32GeU => ("i32.ge_u", I32, I32, 2, false), I64GeU => ("i64.ge_u", I64, I32, 2, false),
    I64ExtendI32S => ("i64.extend_i32_s", I32, I64, 1, false),
    I64ExtendI32U => ("i64.extend_i32_u", I32, I64, 1, false),
    I32WrapI64 => ("i32.wrap_i64", I64, I32, 1, false),
    F32Add => ("f32.add", F32, F32, 2, false), F64Add => ("f64.add", F64, F64, 2, false),
    F32Sub => ("f32.sub", F32, F32, 2, false), F64Sub => ("f64.sub", F64, F64, 2, false),
    F32Mul => ("f32.mul", F32, F32, 2, false), F64Mul => ("f64.mul", F64, F64, 2, false),
    F32Div => ("f32.div", F32, F32, 2, false), F64Div => ("f64.div", F64, F64, 2, false),
    F32Neg => ("f32.neg", F32, F32, 1, false), F64Neg => ("f64.neg", F64, F64, 1, false),
    F32Eq => ("f32.eq", F32, I32, 2, false), F64Eq => ("f64.eq", F64, I32, 2, false),
    F32Ne => ("f32.ne", F32, I32, 2, false), F64Ne => ("f64.ne", F64, I32, 2, false),
    F32Lt => ("f32.lt", F32, I32, 2, false), F64Lt => ("f64.lt", F64, I32, 2, false),
    F32Le => ("f32.le", F32, I32, 2, false), F64Le => ("f64.le", F64, I32, 2, false),
    F32Gt => ("f32.gt", F32, I32, 2, false), F64Gt => ("f64.gt", F64, I32, 2, false),
    F32Ge => ("f32.ge", F32, I32, 2, false), F64Ge => ("f64.ge", F64, I32, 2, false),
}

#[derive(Clone, Debug, Serialize)]
pub struct Field {
    pub name: Option<String>,
    pub ty: Type,
    pub mutable: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Variant {
    pub name: String,
    pub payloads: Vec<Type>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ValueType {
    Bytes,
    Text,
    Option(Type),
    Result {
        ok: Type,
        err: Type,
    },
    Pair {
        first: Type,
        second: Type,
    },
    /// An owned box of shared data. Its identity and storage are private.
    Own(Type),
    Future(Type),
    Stream(Type),
}

impl ValueType {
    /// Private storage cases. Source match never exposes delivery endpoints.
    /// self_type supplies the recursive tail of a finite readable stream.
    pub fn variants(&self, self_type: TypeId) -> Vec<Variant> {
        let payload = |ty: &Type| {
            if *ty == Type::Unit {
                vec![]
            } else {
                vec![ty.clone()]
            }
        };
        match self {
            Self::Bytes | Self::Text => vec![],
            Self::Option(ty) => vec![
                Variant {
                    name: "none".into(),
                    payloads: vec![],
                },
                Variant {
                    name: "some".into(),
                    payloads: payload(ty),
                },
            ],
            Self::Result { ok, err } => vec![
                Variant {
                    name: "ok".into(),
                    payloads: payload(ok),
                },
                Variant {
                    name: "err".into(),
                    payloads: payload(err),
                },
            ],
            Self::Pair { first, second } => vec![Variant {
                name: "pair".into(),
                payloads: [payload(first), payload(second)].concat(),
            }],
            Self::Own(ty) => vec![Variant {
                name: "own".into(),
                payloads: payload(ty),
            }],
            Self::Future(ty) => vec![Variant {
                name: "ready".into(),
                payloads: payload(ty),
            }],
            Self::Stream(ty) => vec![
                Variant {
                    name: "end".into(),
                    payloads: vec![],
                },
                Variant {
                    name: "item".into(),
                    payloads: [payload(ty), vec![Type::Value(self_type)]].concat(),
                },
            ],
        }
    }

    pub fn type_name(&self, format: impl Fn(&Type) -> String) -> String {
        match self {
            Self::Bytes => "bytes".into(),
            Self::Text => "text".into(),
            Self::Option(ty) => format!("(option {})", format(ty)),
            Self::Result { ok, err } => format!("(result {} {})", format(ok), format(err)),
            Self::Pair { first, second } => format!("(pair {} {})", format(first), format(second)),
            Self::Own(ty) => format!("(own {})", format(ty)),
            Self::Future(ty) => format!("(future {})", format(ty)),
            Self::Stream(ty) => format!("(stream {})", format(ty)),
        }
    }
}

/// Local readable-end contracts. Creation is already-ready; reads carry the
/// suspension boundary required by a future host-backed implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum DeliveryOp {
    FutureReady,
    FutureAwait,
    FutureClose,
    StreamEmpty,
    StreamCons,
    StreamNext,
    StreamClose,
}

impl DeliveryOp {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "future.ready" => Self::FutureReady,
            "future.await" => Self::FutureAwait,
            "future.close" => Self::FutureClose,
            "stream.empty" => Self::StreamEmpty,
            "stream.cons" => Self::StreamCons,
            "stream.next" => Self::StreamNext,
            "stream.close" => Self::StreamClose,
            _ => return None,
        })
    }

    pub fn suspends(self) -> bool {
        matches!(self, Self::FutureAwait | Self::StreamNext)
    }
    pub fn allocates(self) -> bool {
        matches!(
            self,
            Self::FutureReady | Self::StreamEmpty | Self::StreamCons | Self::StreamNext
        )
    }
    pub fn may_trap(self) -> bool {
        self.allocates() || self == Self::FutureAwait
    }
    pub fn is_future(self) -> bool {
        matches!(
            self,
            Self::FutureReady | Self::FutureAwait | Self::FutureClose
        )
    }
}

#[derive(Clone, Debug, Serialize)]
pub enum TypeKind {
    Struct(Vec<Field>),
    Array(Field),
    Variant(Vec<Variant>),
    Value(ValueType),
    Closure {
        captures: Vec<Field>,
        function: FunctionId,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct TypeDecl {
    pub name: String,
    pub kind: TypeKind,
}

#[derive(Clone, Debug, Serialize)]
pub enum BindingKind {
    Parameter,
    Capture(usize),
    Let,
    Match,
    Catch,
    Loop(LoopId),
}

#[derive(Clone, Debug, Serialize)]
pub struct Binding {
    pub name: String,
    /// The binding identifier's half-open UTF-8 byte range in the source.
    pub span: Span,
    pub ty: Type,
    pub kind: BindingKind,
}

#[derive(Clone, Debug, Serialize)]
pub enum FunctionKind {
    Defined,
    Import { module: String, name: String },
    Closure { environment: TypeId },
}

#[derive(Clone, Debug, Serialize)]
pub struct Function {
    pub name: String,
    pub kind: FunctionKind,
    pub locals: Vec<Binding>,
    pub params: Vec<LocalId>,
    pub captures: Vec<LocalId>,
    pub result: Type,
    pub exports: Vec<String>,
    pub body: Option<Expr>,
    pub effect_contract: Option<crate::effects::EffectContract>,
    pub import_contract: Option<crate::bindings::Binding>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TestDecl {
    pub name: String,
    pub span: Span,
    pub function: FunctionId,
    pub export: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct LawDecl {
    pub name: String,
    pub span: Span,
    pub function: FunctionId,
}

#[derive(Clone, Debug, Serialize)]
pub struct TagDecl {
    pub name: String,
    pub payloads: Vec<Type>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Module {
    pub types: Vec<TypeDecl>,
    pub functions: Vec<Function>,
    pub tests: Vec<TestDecl>,
    pub laws: Vec<LawDecl>,
    pub tags: Vec<TagDecl>,
}

impl Module {
    pub(crate) fn value_id(&self, value: &ValueType) -> TypeId {
        TypeId(
            self.types
                .iter()
                .position(
                    |declaration| matches!(&declaration.kind, TypeKind::Value(v) if v == value),
                )
                .expect("interned value type"),
        )
    }
    /// Format a type belonging to this checked module using source type names.
    pub fn type_name(&self, ty: &Type) -> String {
        ty.format_with(|id| match &self.types[id.0].kind {
            TypeKind::Value(value) => value.type_name(|ty| self.type_name(ty)),
            _ => self.types[id.0].name.clone(),
        })
    }

    pub(crate) fn instance_type(&self, ty: TypeId) -> Type {
        if matches!(self.types[ty.0].kind, TypeKind::Value(_)) {
            Type::Value(ty)
        } else {
            Type::Ref {
                ty,
                nullable: false,
            }
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Expr {
    pub ty: Type,
    pub span: Span,
    pub kind: ExprKind,
}

#[derive(Clone, Debug, Serialize)]
pub struct MatchArm {
    pub variant: usize,
    pub bindings: Vec<LocalId>,
    pub body: Expr,
}

#[derive(Clone, Debug, Serialize)]
pub struct CatchArm {
    /// None is a final catch_all handler.
    pub tag: Option<TagId>,
    pub bindings: Vec<LocalId>,
    pub body: Expr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum BufferOp {
    New,
    FromArray(TypeId),
    Len,
    Get,
    Slice,
    Encode,
    Decode,
}

impl BufferOp {
    /// Physical allocation and trap contracts; buffers have no observable identity.
    pub fn allocates(self) -> bool {
        matches!(
            self,
            Self::New | Self::FromArray(_) | Self::Slice | Self::Encode | Self::Decode
        )
    }
    pub fn may_trap(self) -> bool {
        self.allocates() || self == Self::Get
    }
}

#[derive(Clone, Debug, Serialize)]
pub enum ExprKind {
    Data(Vec<u8>),
    Buffer {
        op: BufferOp,
        /// Representation of the first buffer operand (the result for New).
        buffer: TypeId,
        args: Vec<Expr>,
    },
    Const {
        ty: NumType,
        literal: String,
    },
    Numeric {
        op: NumericOp,
        args: Vec<Expr>,
    },
    Local(LocalId),
    Move(LocalId),
    Close(Box<Expr>),
    Delivery {
        op: DeliveryOp,
        endpoint: TypeId,
        args: Vec<Expr>,
    },
    Let {
        local: LocalId,
        value: Box<Expr>,
        body: Box<Expr>,
    },
    Block(Vec<Expr>),
    If {
        condition: Box<Expr>,
        then_body: Box<Expr>,
        else_body: Box<Expr>,
    },
    Throw {
        tag: TagId,
        values: Vec<Expr>,
    },
    Try {
        body: Box<Expr>,
        handlers: Vec<CatchArm>,
    },
    VariantNew {
        ty: TypeId,
        variant: usize,
        values: Vec<Expr>,
    },
    Match {
        ty: TypeId,
        subject: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    Loop {
        label: LoopId,
        bindings: Vec<(LocalId, Expr)>,
        body: Box<Expr>,
    },
    Recur {
        label: LoopId,
        assignments: Vec<(LocalId, Expr)>,
    },
    Call {
        function: FunctionId,
        args: Vec<Expr>,
    },
    StructNew {
        ty: TypeId,
        values: Vec<Expr>,
    },
    StructGet {
        ty: TypeId,
        field: usize,
        object: Box<Expr>,
    },
    StructSet {
        ty: TypeId,
        field: usize,
        object: Box<Expr>,
        value: Box<Expr>,
    },
    ArrayNew {
        ty: TypeId,
        initial: Box<Expr>,
        length: Box<Expr>,
    },
    ArrayFixed {
        ty: TypeId,
        values: Vec<Expr>,
    },
    ArrayGet {
        ty: TypeId,
        object: Box<Expr>,
        index: Box<Expr>,
    },
    ArraySet {
        ty: TypeId,
        object: Box<Expr>,
        index: Box<Expr>,
        value: Box<Expr>,
    },
    ArrayLen(Box<Expr>),
    Null(TypeId),
    IsNull(Box<Expr>),
    AsNonNull(Box<Expr>),
    RefEq(Box<Expr>, Box<Expr>),
    Drop(Box<Expr>),
    Nop,
    Unreachable,
}

impl Expr {
    pub fn children(&self) -> Vec<&Expr> {
        use ExprKind::*;
        match &self.kind {
            Buffer { args, .. }
            | Numeric { args, .. }
            | Call { args, .. }
            | Delivery { args, .. } => args.iter().collect(),
            StructNew { values, .. }
            | VariantNew { values, .. }
            | Throw { values, .. }
            | ArrayFixed { values, .. }
            | Block(values) => values.iter().collect(),
            Match { subject, arms, .. } => std::iter::once(subject.as_ref())
                .chain(arms.iter().map(|arm| &arm.body))
                .collect(),
            Try { body, handlers } => std::iter::once(body.as_ref())
                .chain(handlers.iter().map(|handler| &handler.body))
                .collect(),
            Let { value, body, .. } => vec![value, body],
            If {
                condition,
                then_body,
                else_body,
            } => vec![condition, then_body, else_body],
            Loop { bindings, body, .. } => bindings
                .iter()
                .map(|(_, e)| e)
                .chain([body.as_ref()])
                .collect(),
            Recur { assignments, .. } => assignments.iter().map(|(_, e)| e).collect(),
            StructGet { object, .. }
            | ArrayLen(object)
            | IsNull(object)
            | AsNonNull(object)
            | Close(object)
            | Drop(object) => vec![object],
            StructSet { object, value, .. } => vec![object, value],
            ArrayNew {
                initial, length, ..
            } => vec![initial, length],
            ArrayGet { object, index, .. } => vec![object, index],
            ArraySet {
                object,
                index,
                value,
                ..
            } => vec![object, index, value],
            RefEq(a, b) => vec![a, b],
            Data(_) | Const { .. } | Local(_) | Move(_) | Null(_) | Nop | Unreachable => vec![],
        }
    }
}
