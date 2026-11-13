//! Closed type descriptions for a fresh implementation type scope. Source
//! nominals retain distinct cache identities; no foreign heap/resource IDs or
//! runtime values cross a Component boundary through a type argument.
use super::*;
use crate::{interface::AbiType, ir::*};

#[derive(Clone)]
pub(super) struct Snapshot {
    pub arguments: Vec<Sexp>,
    pub declarations: Vec<Sexp>,
    pub key: Vec<String>,
    pub labels: BTreeMap<String, String>,
}
pub(super) fn build(
    source: &str,
    request: &Sexp,
    module: &Module,
    arguments: &[Type],
    origins: &BTreeMap<String, String>,
    opaque: &BTreeSet<String>,
) -> Result<Snapshot, Error> {
    let mut prefix = "$walz-constructor-type".to_owned();
    while source.contains(&prefix) {
        prefix.push('x');
    }
    let mut w = Writer {
        module,
        prefix,
        origins,
        opaque,
        rejected: None,
        names: BTreeMap::new(),
        definitions: BTreeMap::new(),
        identities: vec![],
        visits: 0,
        bytes: 0,
        labels: Default::default(),
    };
    let types = arguments.iter().map(|t| w.ty(t, 0)).collect::<Option<Vec<_>>>().ok_or_else(|| {
        if let Some(name) = &w.rejected {
            return Error::at(source,request.span,format!("opaque type `{name}` cannot be copied into a Component constructor; use its defining module's public functions or an explicit instance adapter"))
                .with_code("E_OPAQUE_REPRESENTATION");
        }
        fail(source, request,
            "Component type arguments need closed type definitions; resource/borrow and closure identities require explicit runtime connections (type graph limit: 64 levels, 1024 nominal definitions, 65536 visits, 16 MiB)")
    })?;
    let mut arguments = vec![];
    for ty in &types {
        arguments.push(crate::sexpr::parse(ty)?);
    }
    let definitions = w.definitions.into_values().collect::<Vec<_>>().join("\n");
    let root = crate::sexpr::parse(&format!("(module {definitions})"))?;
    let declarations = root.list().unwrap()[1..].to_vec();
    let mut key = types;
    key.extend(w.identities);
    key.push(definitions);
    Ok(Snapshot {
        arguments,
        declarations,
        key,
        labels: w.labels,
    })
}
struct Writer<'a> {
    module: &'a Module,
    prefix: String,
    origins: &'a BTreeMap<String, String>,
    opaque: &'a BTreeSet<String>,
    rejected: Option<String>,
    names: BTreeMap<TypeId, String>,
    definitions: BTreeMap<usize, String>,
    identities: Vec<String>,
    visits: usize,
    bytes: usize,
    labels: BTreeMap<String, String>,
}
impl Writer<'_> {
    fn ty(&mut self, ty: &Type, depth: usize) -> Option<String> {
        self.visits += 1;
        if depth > 64 || self.visits > 65536 {
            return None;
        }
        let text = match ty {
            Type::Num(n) => n.wat().into(),
            Type::Unit => "@unit".into(),
            Type::Newtype(id) => self.nominal(*id, depth)?,
            Type::Ref { ty, nullable } => format!(
                "(ref {}{})",
                if *nullable { "null " } else { "" },
                self.nominal(*ty, depth)?
            ),
            Type::Value(id) => match self.module.types[id.0].kind.clone() {
                TypeKind::Value(ValueType::Bytes) => "@bytes".into(),
                TypeKind::Value(ValueType::Text) => "@string".into(),
                TypeKind::Value(v) => {
                    let (name, children) = match &v {
                        ValueType::List(t) => ("list", vec![t]),
                        ValueType::Option(t) => ("option", vec![t]),
                        ValueType::Pair { first, second } => ("tuple", vec![first, second]),
                        ValueType::Result { ok, err } => ("result", vec![ok, err]),
                        ValueType::Future(t) => ("future", vec![t]),
                        ValueType::Stream(t) => ("stream", vec![t]),
                        ValueType::Own(t) => ("box", vec![t]),
                        ValueType::Task(t) => ("task", vec![t]),
                        _ => unreachable!(),
                    };
                    let children = children
                        .into_iter()
                        .map(|t| self.ty(t, depth + 1))
                        .collect::<Option<Vec<_>>>()?;
                    format!("(@{name} {})", children.join(" "))
                }
                _ => return None,
            },
            _ => return None,
        };
        self.bytes = self.bytes.saturating_add(text.len());
        (self.bytes <= 16 * 1024 * 1024).then_some(text)
    }
    fn field(&mut self, f: &Field, depth: usize) -> Option<String> {
        let ty = if let Some(p) = f.packed {
            p.wat().into()
        } else {
            self.ty(&f.ty, depth + 1)?
        };
        let storage = if f.mutable { format!("(mut {ty})") } else { ty };
        Some(format!(
            "(field {}{storage})",
            f.name.as_ref().map(|n| format!("{n} ")).unwrap_or_default()
        ))
    }
    fn nominal(&mut self, id: TypeId, depth: usize) -> Option<String> {
        if let Some(name) = self.names.get(&id) {
            return Some(name.clone());
        }
        if self.names.len() >= 1024 {
            return None;
        }
        let slot = self.names.len();
        let name = format!("{}.t{slot}", self.prefix);
        self.names.insert(id, name.clone());
        let decl = self.module.types[id.0].clone();
        if self.opaque.contains(&decl.name) {
            self.rejected = Some(decl.name);
            return None;
        }
        self.labels.insert(
            name.clone(),
            self.origins
                .get(&decl.name)
                .and_then(|n| n.strip_prefix("wit:").or_else(|| n.strip_prefix("source:")))
                .unwrap_or(&decl.name)
                .to_owned(),
        );
        self.identities.push(
            self.origins
                .get(&decl.name)
                .cloned()
                .unwrap_or_else(|| format!("source:{}", decl.name)),
        );
        let definition = match &decl.kind {
            TypeKind::Newtype(n) => format!("(@newtype {})", n.wat()),
            TypeKind::Struct(fields) => format!(
                "(struct {})",
                fields
                    .iter()
                    .map(|f| self.field(f, depth))
                    .collect::<Option<Vec<_>>>()?
                    .join(" ")
            ),
            TypeKind::Array(f) => {
                let field = self.field(f, depth)?;
                // Array storage has no field-name binder.
                let field = crate::sexpr::parse(&field).ok()?;
                format!("(array {})", render(field.list()?.last()?))
            }
            TypeKind::Variant(cases) => {
                let cases = cases
                    .iter()
                    .map(|c| {
                        let payloads = c
                            .payloads
                            .iter()
                            .map(|t| self.ty(t, depth + 1))
                            .collect::<Option<Vec<_>>>()?;
                        Some(format!("({} {})", c.name, payloads.join(" ")))
                    })
                    .collect::<Option<Vec<_>>>()?;
                format!("(@variant {})", cases.join(" "))
            }
            _ => return None,
        };
        let mut text = format!("(type {name} {definition})");
        if let Some(abi) = &decl.abi {
            text.push_str(&format!("\n(@abi.type {name} {})", annotation(abi)));
        }
        self.bytes = self.bytes.saturating_add(text.len());
        if self.bytes > 16 * 1024 * 1024 {
            return None;
        }
        self.definitions.insert(slot, text);
        Some(name)
    }
}
fn annotation(a: &AbiType) -> String {
    let (name, names): (&str, &[String]) = match a {
        AbiType::Bool => ("bool", &[]),
        AbiType::Char => ("char", &[]),
        AbiType::S8 => ("s8", &[]),
        AbiType::S16 => ("s16", &[]),
        AbiType::U8 => ("u8", &[]),
        AbiType::U16 => ("u16", &[]),
        AbiType::U32 => ("u32", &[]),
        AbiType::U64 => ("u64", &[]),
        AbiType::Tuple => ("tuple", &[]),
        AbiType::Record(n) => ("record", n),
        AbiType::Variant(n) => ("variant", n),
        AbiType::Enum(n) => ("enum", n),
        AbiType::Flags(n) => ("flags", n),
    };
    format!(
        "({name}{})",
        names
            .iter()
            .map(|n| format!(" {}", crate::binary::quoted(n.as_bytes())))
            .collect::<String>()
    )
}

pub(super) fn inject(body: &mut Sexp, declarations: &[Sexp]) {
    if declarations.is_empty() || body.head().is_some_and(crate::syntax::annotation) {
        return;
    }
    let component = body.head() == Some("component");
    if let Kind::List(p) = &mut body.kind {
        for n in p.iter_mut() {
            inject(n, declarations);
        }
        if component {
            let at = if p.get(1).is_some_and(|n| n.atom().is_some()) {
                2
            } else {
                1
            };
            let head = Sexp {
                kind: Kind::Atom("@lib".into()),
                span: body.span,
                context: Default::default(),
            };
            let lib = Sexp {
                kind: Kind::List(
                    std::iter::once(head)
                        .chain(declarations.iter().cloned())
                        .collect(),
                ),
                span: body.span,
                context: Default::default(),
            };
            if let Kind::List(p) = &mut body.kind {
                p.insert(at, lib);
            }
        }
    }
}
