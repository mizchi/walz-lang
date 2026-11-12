//! Stable read-only macro type information. Nominal names are snapshot-local
//! identities; their spelling and private representation are not a contract.
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum TypeInfo {
    Scalar { name: String },
    Unit,
    Bytes,
    String,
    List { element: Box<Self> },
    Option { element: Box<Self> },
    Result { ok: Box<Self>, err: Box<Self> },
    Tuple { first: Box<Self>, second: Box<Self> },
    Box { element: Box<Self> },
    Future { element: Box<Self> },
    Stream { element: Box<Self> },
    Task { element: Box<Self> },
    Ref { name: String, nullable: bool },
    Nominal { name: String },
    Resource { name: String },
    Borrow { name: String },
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Projection {
    #[default]
    Identity,
    OptionElement,
    ListElement,
    ResultOk,
    ResultErr,
    TupleFirst,
    TupleSecond,
    BoxElement,
    FutureElement,
    StreamElement,
    TaskElement,
    ArrayElement,
    RefHeap,
}
impl Projection {
    pub fn name(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::OptionElement => "option.element",
            Self::ListElement => "list.element",
            Self::ResultOk => "result.ok",
            Self::ResultErr => "result.err",
            Self::TupleFirst => "tuple.first",
            Self::TupleSecond => "tuple.second",
            Self::BoxElement => "box.element",
            Self::FutureElement => "future.element",
            Self::StreamElement => "stream.element",
            Self::TaskElement => "task.element",
            Self::ArrayElement => "array.element",
            Self::RefHeap => "ref.heap",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "option.element" => Self::OptionElement,
            "list.element" => Self::ListElement,
            "result.ok" => Self::ResultOk,
            "result.err" => Self::ResultErr,
            "tuple.first" => Self::TupleFirst,
            "tuple.second" => Self::TupleSecond,
            "box.element" => Self::BoxElement,
            "future.element" => Self::FutureElement,
            "stream.element" => Self::StreamElement,
            "task.element" => Self::TaskElement,
            "array.element" => Self::ArrayElement,
            "ref.heap" => Self::RefHeap,
            _ => return None,
        })
    }
}
