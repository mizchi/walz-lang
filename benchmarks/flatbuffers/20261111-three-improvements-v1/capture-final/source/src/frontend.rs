use crate::Error;
use crate::ir::*;
use crate::sexpr::Sexp;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

mod generic_calls;
mod high_level;
mod lexical;
mod macro_expansion;
mod macro_types;

type Names = BTreeMap<String, usize>;
type Scope = BTreeMap<String, LocalId>;

// Identifiers are ASCII. Bound both edit distance and identifier length so a
// diagnostic cannot turn a very long name into quadratic checker work.
fn name_distance(left: &str, right: &str) -> Option<usize> {
    if left.len() > 64 || right.len() > 64 || left.len().abs_diff(right.len()) > 2 {
        return None;
    }
    let mut row = (0..=right.len()).collect::<Vec<_>>();
    for (i, a) in left.bytes().enumerate() {
        let mut next = vec![i + 1];
        for (j, b) in right.bytes().enumerate() {
            next.push(
                (row[j + 1] + 1)
                    .min(next[j] + 1)
                    .min(row[j] + usize::from(a != b)),
            );
        }
        if next.iter().all(|&distance| distance > 2) {
            return None;
        }
        row = next;
    }
    let distance = row[right.len()];
    (distance <= if left.len() <= 5 { 1 } else { 2 }).then_some(distance)
}

struct FunctionSite {
    name: Span,
    result: Option<Span>,
}

struct Tables<'a> {
    source: &'a str,
    macros: Option<&'a crate::macros::Environment>,
    inference: Option<&'a crate::generics::Inference>,
    aliases: Option<&'a crate::aliases::Expanded>,
    privacy: Option<(&'a crate::modules::Expanded, &'a BTreeMap<String, String>)>,
    types: Names,
    newtypes: BTreeMap<TypeId, NumType>,
    functions: Names,
    memories: Names,
    atomic_cells: Names,
    readonly_regions: Names,
    function_sites: BTreeMap<String, FunctionSite>,
    tags: Names,
    values: RefCell<ValueTables>,
}

#[derive(Default)]
struct ValueTables {
    ids: BTreeMap<ValueType, TypeId>,
    types: Vec<TypeDecl>,
}

impl Tables<'_> {
    fn new(source: &str) -> Tables<'_> {
        Tables {
            source,
            macros: None,
            inference: None,
            aliases: None,
            privacy: None,
            types: Names::new(),
            newtypes: BTreeMap::new(),
            functions: Names::new(),
            memories: Names::new(),
            atomic_cells: Names::new(),
            readonly_regions: Names::new(),
            function_sites: BTreeMap::new(),
            tags: Names::new(),
            values: RefCell::default(),
        }
    }

    fn error(&self, node: &Sexp, message: impl std::fmt::Display) -> Error {
        Error::at(self.source, node.span, message)
    }

    fn atom<'a>(&self, node: &'a Sexp) -> Result<&'a str, Error> {
        node.atom().ok_or_else(|| self.error(node, "expected atom"))
    }

    fn list<'a>(&self, node: &'a Sexp) -> Result<&'a [Sexp], Error> {
        node.list()
            .ok_or_else(|| self.error(node, "expected folded expression"))
    }

    fn name(&self, node: &Sexp) -> Result<String, Error> {
        let name = self.atom(node)?;
        if name.starts_with("$__") {
            return Err(self.error(node, "the $__ prefix is reserved"));
        }
        if !name.starts_with('$')
            || name.len() == 1
            || !name[1..]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
        {
            return Err(self.error(node, "expected ASCII $identifier"));
        }
        Ok(name.to_owned())
    }

    fn count(&self, node: &Sexp, items: &[Sexp], count: usize) -> Result<(), Error> {
        if items.len() == count {
            Ok(())
        } else {
            Err(self.error(
                node,
                format!(
                    "argument count: expected {}, got {}",
                    count - 1,
                    items.len().saturating_sub(1)
                ),
            ))
        }
    }

    fn variant_name(&self, node: &Sexp) -> Result<String, Error> {
        let name = self.atom(node)?;
        if !name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
        {
            return Err(self.error(node, "expected ASCII variant identifier"));
        }
        Ok(name.to_owned())
    }

    fn type_id(&self, node: &Sexp) -> Result<TypeId, Error> {
        let name = self.atom(node)?;
        self.types
            .get(name)
            .copied()
            .map(TypeId)
            .ok_or_else(|| self.error(node, format!("unknown type `{name}`")))
    }

    fn tag_id(&self, node: &Sexp) -> Result<TagId, Error> {
        let name = self.atom(node)?;
        self.tags
            .get(name)
            .copied()
            .map(TagId)
            .ok_or_else(|| self.error(node, format!("unknown tag `{name}`")))
    }

    fn ty(&self, node: &Sexp) -> Result<Type, Error> {
        if let Some(ty) = node.atom().and_then(NumType::parse) {
            return Ok(Type::Num(ty));
        }
        if let Some(value) = self.value_shape(node)? {
            return Ok(Type::Value(self.intern_value(value)));
        }
        if node.atom().is_some() {
            if let Some(id) = node
                .atom()
                .and_then(|n| self.types.get(n))
                .copied()
                .map(TypeId)
                && self.newtypes.contains_key(&id)
            {
                return Ok(Type::Newtype(id));
            }
            return Err(self.error(
                node,
                "expected numeric type, reference, bytes, text, or constructed value type",
            ));
        }
        let items = self.list(node)?;
        if items.first().and_then(Sexp::atom) == Some("ref")
            && let Some(name) = items.last()
            && let Ok(id) = self.type_id(name)
            && self.newtypes.contains_key(&id)
        {
            return Err(self.error(node, "newtype is a scalar, not a GC heap type"));
        }
        match items {
            [head, name] if matches!(head.atom(), Some("resource" | "borrow")) => {
                let id = self.type_id(name)?;
                Ok(if head.atom() == Some("borrow") {
                    Type::Borrow(id)
                } else {
                    Type::Resource(id)
                })
            }
            [head, name] if head.atom() == Some("ref") => Ok(Type::Ref {
                ty: self.type_id(name)?,
                nullable: false,
            }),
            [head, null, name] if head.atom() == Some("ref") && null.atom() == Some("null") => {
                Ok(Type::Ref {
                    ty: self.type_id(name)?,
                    nullable: true,
                })
            }
            _ => Err(self.error(node, "expected numeric type or (ref [null] $Type)")),
        }
    }

    fn payload_ty(&self, node: &Sexp) -> Result<Type, Error> {
        if node.atom() == Some("unit") {
            Ok(Type::Unit)
        } else {
            self.ty(node)
        }
    }

    fn value_shape(&self, node: &Sexp) -> Result<Option<ValueType>, Error> {
        if node.atom() == Some("bytes") {
            return Ok(Some(ValueType::Bytes));
        }
        if node.atom() == Some("text") {
            return Ok(Some(ValueType::Text));
        }
        let Some(items) = node.list() else {
            return Ok(None);
        };
        Ok(match node.head() {
            Some("list") => {
                self.count(node, items, 2)?;
                Some(ValueType::List(self.payload_ty(&items[1])?))
            }
            Some("option") => {
                self.count(node, items, 2)?;
                Some(ValueType::Option(self.payload_ty(&items[1])?))
            }
            Some("own") => {
                self.count(node, items, 2)?;
                Some(ValueType::Own(self.payload_ty(&items[1])?))
            }
            Some("future" | "stream" | "task") => {
                self.count(node, items, 2)?;
                let payload = self.payload_ty(&items[1])?;
                Some(if node.head() == Some("task") {
                    ValueType::Task(payload)
                } else if node.head() == Some("future") {
                    ValueType::Future(payload)
                } else {
                    ValueType::Stream(payload)
                })
            }
            Some("pair") => {
                self.count(node, items, 3)?;
                Some(ValueType::Pair {
                    first: self.payload_ty(&items[1])?,
                    second: self.payload_ty(&items[2])?,
                })
            }
            // Two operands distinguish the type constructor from (result T).
            Some("result") if items.len() == 3 => Some(ValueType::Result {
                ok: self.payload_ty(&items[1])?,
                err: self.payload_ty(&items[2])?,
            }),
            _ => None,
        })
    }

    fn intern_value(&self, value: ValueType) -> TypeId {
        if let Some(id) = self.values.borrow().ids.get(&value) {
            return *id;
        }
        let id = TypeId(self.types.len() + self.values.borrow().types.len());
        self.values.borrow_mut().ids.insert(value.clone(), id);
        self.values.borrow_mut().types.push(TypeDecl {
            abi: None,
            abi_facts: None,
            name: format!("$__value{}", id.0),
            kind: TypeKind::Value(value.clone()),
        });
        if let ValueType::Stream(payload) = value {
            let option = self.intern_value(ValueType::Option(payload));
            self.intern_value(ValueType::Pair {
                first: Type::Value(id),
                second: Type::Value(option),
            });
        }
        id
    }

    fn collect_values(&self, node: &Sexp) -> Result<(), Error> {
        if node.head() == Some("abi.type") {
            return Ok(());
        }
        if node.atom() == Some("text") {
            self.intern_value(ValueType::Bytes);
        }
        if matches!(node.atom(), Some("bytes" | "text")) {
            self.intern_value(if node.atom() == Some("bytes") {
                ValueType::Bytes
            } else {
                ValueType::Text
            });
        }
        if node
            .head()
            .is_some_and(|h| h.starts_with("bytes.") || h.starts_with("text."))
        {
            self.intern_value(ValueType::Bytes);
            let text = self.intern_value(ValueType::Text);
            if node.head() == Some("text.from-bytes") {
                self.intern_value(ValueType::Option(Type::Value(text)));
            }
        }
        let Some(items) = node.list() else {
            return Ok(());
        };
        if node.head() == Some("variant") {
            for case in &items[1..] {
                if let Some(parts) = case.list() {
                    for payload in parts.iter().skip(1) {
                        self.collect_values(payload)?;
                    }
                }
            }
            return Ok(());
        }
        for child in items {
            self.collect_values(child)?;
        }
        if let Some(value) = self.value_shape(node)? {
            self.intern_value(value);
        }
        match node.head() {
            Some(
                "task.spawn" | "task.join" | "task.drop" | "task.cancel" | "task.join-timeout",
            ) if items.len() >= 2 => {
                self.intern_value(ValueType::Task(self.payload_ty(&items[1])?));
                if node.head() == Some("task.join-timeout") {
                    self.intern_value(ValueType::Option(self.payload_ty(&items[1])?));
                }
            }
            Some(name) if name.starts_with("list.") && items.len() >= 2 => {
                self.intern_value(ValueType::List(self.payload_ty(&items[1])?));
            }
            Some(name) if DeliveryOp::parse(name).is_some() && items.len() >= 2 => {
                let op = DeliveryOp::parse(name).unwrap();
                let payload = self.payload_ty(&items[1])?;
                self.intern_value(if op.is_future() {
                    ValueType::Future(payload)
                } else {
                    ValueType::Stream(payload)
                });
            }
            Some("option.none" | "option.some")
                if items.get(1).is_some_and(|n| self.is_payload_syntax(n)) =>
            {
                self.intern_value(ValueType::Option(self.payload_ty(&items[1])?));
            }
            Some("result.ok" | "result.err")
                if items.len() >= 3
                    && self.is_payload_syntax(&items[1])
                    && self.is_payload_syntax(&items[2]) =>
            {
                self.intern_value(ValueType::Result {
                    ok: self.payload_ty(&items[1])?,
                    err: self.payload_ty(&items[2])?,
                });
            }
            Some("own.new") if items.get(1).is_some_and(|n| self.is_payload_syntax(n)) => {
                self.intern_value(ValueType::Own(self.payload_ty(&items[1])?));
            }
            Some("pair.new")
                if items.len() >= 3
                    && self.is_payload_syntax(&items[1])
                    && self.is_payload_syntax(&items[2]) =>
            {
                self.intern_value(ValueType::Pair {
                    first: self.payload_ty(&items[1])?,
                    second: self.payload_ty(&items[2])?,
                });
            }
            _ => {}
        }
        Ok(())
    }

    fn is_payload_syntax(&self, node: &Sexp) -> bool {
        node.atom().is_some_and(|s| {
            s == "unit"
                || s == "bytes"
                || s == "text"
                || NumType::parse(s).is_some()
                || self
                    .types
                    .get(s)
                    .is_some_and(|id| self.newtypes.contains_key(&TypeId(*id)))
        }) || matches!(
            node.head(),
            Some(
                "ref"
                    | "resource"
                    | "borrow"
                    | "list"
                    | "option"
                    | "own"
                    | "future"
                    | "stream"
                    | "task"
                    | "pair"
            )
        ) || node.head() == Some("result") && node.list().unwrap().len() == 3
    }

    fn value(&self, ty: &Type) -> Option<ValueType> {
        let Type::Value(id) = ty else {
            return None;
        };
        let index = id.0.checked_sub(self.types.len())?;
        match &self.values.borrow().types.get(index)?.kind {
            TypeKind::Value(value) => Some(value.clone()),
            _ => None,
        }
    }

    fn field(&self, name: Option<String>, node: &Sexp) -> Result<Field, Error> {
        let (ty, mutable) = if node.head() == Some("mut") {
            let items = self.list(node)?;
            self.count(node, items, 2)?;
            (self.ty(&items[1])?, true)
        } else {
            (self.ty(node)?, false)
        };
        Ok(Field {
            name,
            ty,
            mutable,
            packed: None,
        })
    }

    fn type_name(&self, ty: &Type) -> String {
        ty.format_with(|id| {
            if let Some(declaration) =
                id.0.checked_sub(self.types.len())
                    .and_then(|i| self.values.borrow().types.get(i).cloned())
            {
                let TypeKind::Value(value) = &declaration.kind else {
                    unreachable!()
                };
                value.type_name(|ty| self.type_name(ty))
            } else {
                self.types
                    .iter()
                    .find(|(_, index)| **index == id.0)
                    .expect("checked type ID")
                    .0
                    .clone()
            }
        })
    }

    fn effect_contract(&self, node: &Sexp) -> Result<crate::effects::EffectContract, Error> {
        let items = self.list(node)?;
        let mut allowed = crate::effects::EffectSet::default();
        let mut seen = BTreeSet::new();
        for item in &items[1..] {
            let name = item
                .atom()
                .or_else(|| item.head())
                .ok_or_else(|| self.error(item, "expected effect"))?;
            if item.atom().is_none() && name != "throws" {
                return Err(self.error(item, "expected effect name or (throws tags...)"));
            }
            if !seen.insert(name) {
                return Err(self.error(item, "duplicate effect"));
            }
            match name {
                "pure" if items.len() != 2 => {
                    return Err(self.error(item, "pure must be used alone"));
                }
                "pure" => {}
                "reads" => allowed.reads = true,
                "writes" => allowed.writes = true,
                "allocates" => allowed.allocates = true,
                "suspends" => allowed.suspends = true,
                "host" => allowed.host = true,
                "synchronizes" => allowed.synchronizes = true,
                "blocks" => allowed.blocks = true,
                "throws" => {
                    let tags = self.list(item)?;
                    if tags.len() == 2 && tags[1].atom() == Some("*") {
                        allowed.throws_unknown = true;
                    } else {
                        for tag in &tags[1..] {
                            if !allowed.throws.insert(self.tag_id(tag)?) {
                                return Err(self.error(tag, "duplicate throws tag"));
                            }
                        }
                    }
                }
                _ => return Err(self.error(item, format!("unknown effect `{name}`"))),
            }
        }
        Ok(crate::effects::EffectContract {
            span: node.span,
            allowed,
        })
    }

    fn expect_type(&self, node: &Sexp, expected: &Type, actual: &Type) -> Result<(), Error> {
        if expected.accepts(actual) {
            Ok(())
        } else {
            Err(self
                .error(
                    node,
                    format!(
                        "type mismatch: expected {}, got {}",
                        self.type_name(expected),
                        self.type_name(actual)
                    ),
                )
                .with_code("E_TYPE_MISMATCH"))
        }
    }

    fn branch<'a>(&self, node: &'a Sexp, expected: &str) -> Result<&'a Sexp, Error> {
        let parts = self.list(node)?;
        self.count(node, parts, 2)?;
        if node.head() != Some(expected) {
            return Err(self.error(node, format!("expected ({expected} expression)")));
        }
        Ok(&parts[1])
    }
}

/// Reuse the ordinary effect grammar for unapplied constraint declarations.
pub(crate) fn validate_effect_syntax(source: &str, node: &Sexp) -> Result<(), Error> {
    let mut tables = Tables::new(source);
    for item in node.list().unwrap_or(&[]).iter().skip(1) {
        if item.head() == Some("throws") {
            for tag in item.list().unwrap().iter().skip(1) {
                if tag.atom() != Some("*") {
                    let name = tables.name(tag)?;
                    let next = tables.tags.len();
                    tables.tags.entry(name).or_insert(next);
                }
            }
        }
    }
    tables.effect_contract(node).map(|_| ())
}

pub(crate) fn check<'b>(
    source: &str,
    root: &Sexp,
    bounds: &'b [crate::bounds::Site],
    macros: &crate::macros::Environment,
    inference: &crate::generics::Inference,
    privacy: (&crate::modules::Expanded, &BTreeMap<String, String>),
    aliases: &crate::aliases::Expanded,
) -> Result<(Module, Vec<crate::bounds::EffectObligation<'b>>), Error> {
    let mut tables = Tables::new(source);
    tables.macros = Some(macros);
    macros.begin_check(root);
    tables.inference = Some(inference);
    tables.aliases = Some(aliases);
    tables.privacy = Some(privacy);
    let declarations = tables.list(root)?;
    if root.head() != Some("module") {
        return Err(tables.error(root, "expected (module ...)"));
    }
    let mut type_nodes = Vec::new();
    let mut function_nodes = Vec::new();
    let mut test_nodes = Vec::new();
    let mut bench_nodes = Vec::new();
    let mut bench_names = BTreeSet::new();
    let mut law_nodes = Vec::new();
    let mut law_names = BTreeSet::new();
    let mut test_names = BTreeSet::new();
    let mut tag_nodes = Vec::new();
    let mut memory_nodes = Vec::new();
    let mut cell_nodes = Vec::new();
    let mut region_nodes = Vec::new();
    let mut abi_nodes = Vec::new();
    let mut components = Vec::new();
    let mut race_mode = None;
    for node in &declarations[1..] {
        let items = tables.list(node)?;
        match node.head() {
            Some("component.binary") => components.push(crate::binary::declaration(source, node)?),
            Some("type" | "closure") => {
                let name_node = items
                    .get(1)
                    .ok_or_else(|| tables.error(node, "missing declaration name"))?;
                let name = tables.name(name_node)?;
                if tables.types.insert(name, type_nodes.len()).is_some() {
                    return Err(tables.error(node, "duplicate type"));
                }
                type_nodes.push(node);
                if node.head() == Some("closure") {
                    function_nodes.push((node, None));
                }
            }
            Some("func") => function_nodes.push((node, None)),
            Some("memory") => memory_nodes.push((node, None)),
            Some("atomic.cell") => cell_nodes.push(node),
            Some("readonly.region") => region_nodes.push(node),
            Some("abi.type") => abi_nodes.push(node),
            Some("race-free") => {
                if items.len() != 1 || race_mode.replace(node.span).is_some() {
                    return Err(tables
                        .error(node, "expected one (@race-free) declaration")
                        .with_code("E_RACE_MODE"));
                }
            }
            Some("tag") => {
                let name = tables.name(
                    items
                        .get(1)
                        .ok_or_else(|| tables.error(node, "missing tag name"))?,
                )?;
                if tables.tags.insert(name, tag_nodes.len()).is_some() {
                    return Err(tables.error(node, "duplicate tag"));
                }
                tag_nodes.push(node);
            }
            Some("test") => {
                if items.len() != 3 {
                    return Err(tables.error(node, "expected (test \"name\" i32-expression)"));
                }
                let name = items[1]
                    .text()
                    .ok_or_else(|| tables.error(&items[1], "expected test name string"))?;
                if name.trim().is_empty() {
                    return Err(tables.error(&items[1], "expected non-empty test name"));
                }
                if !test_names.insert(name.to_owned()) {
                    return Err(tables.error(&items[1], "duplicate test name"));
                }
                test_nodes.push(node);
            }
            Some("bench") => {
                let name = items
                    .get(1)
                    .and_then(Sexp::text)
                    .ok_or_else(|| tables.error(node, "expected benchmark name string"))?;
                if name.trim().is_empty() {
                    return Err(tables.error(&items[1], "expected non-empty benchmark name"));
                }
                if !bench_names.insert(name.to_owned()) {
                    return Err(tables.error(&items[1], "duplicate benchmark name"));
                }
                bench_nodes.push(node);
            }
            Some("law") => {
                if items.len() < 3 {
                    return Err(tables.error(
                        node,
                        "expected (law \"name\" (param $name type)... i32-expression)",
                    ));
                }
                let name = items[1]
                    .text()
                    .ok_or_else(|| tables.error(&items[1], "expected law name string"))?;
                if !law_names.insert(name.to_owned()) {
                    return Err(tables.error(&items[1], "duplicate law"));
                }
                law_nodes.push(node);
            }
            Some("import") => {
                tables.count(node, items, 4)?;
                let module = items[1]
                    .text()
                    .ok_or_else(|| tables.error(node, "expected import module string"))?;
                let name = items[2]
                    .text()
                    .ok_or_else(|| tables.error(node, "expected import name string"))?;
                match items[3].head() {
                    Some("func") => {
                        function_nodes.push((&items[3], Some((module.to_owned(), name.to_owned()))))
                    }
                    Some("memory") => memory_nodes.push((
                        &items[3],
                        Some(crate::threads::MemoryImport {
                            module: module.into(),
                            name: name.into(),
                        }),
                    )),
                    _ => {
                        return Err(
                            tables.error(node, "typed imports support functions and memories")
                        );
                    }
                }
            }
            _ => return Err(tables.error(node, "unsupported declaration")),
        }
    }
    // Memory indices count imports before definitions, as in Core Wasm.
    memory_nodes.sort_by_key(|(_, import)| import.is_none());
    let mut memories = Vec::new();
    for (node, import) in memory_nodes {
        let memory = crate::threads::declaration(source, node, import)?;
        tables
            .name(&tables.list(node)?[1])
            .map_err(|e| e.with_code("E_MEMORY_DECL"))?;
        if tables
            .memories
            .insert(memory.name.clone(), memories.len())
            .is_some()
        {
            return Err(tables
                .error(node, "duplicate memory")
                .with_code("E_MEMORY_DECL"));
        }
        memories.push(memory);
    }
    let mut atomic_cells = Vec::new();
    for node in cell_nodes {
        let cell = crate::race::cell_declaration(source, node, &memories)?;
        tables.name(&tables.list(node)?[1])?;
        if tables
            .atomic_cells
            .insert(cell.name.clone(), atomic_cells.len())
            .is_some()
        {
            return Err(tables
                .error(node, "duplicate atomic cell")
                .with_code("E_ATOMIC_CELL"));
        }
        atomic_cells.push(cell);
    }
    let mut readonly_regions = Vec::new();
    for node in region_nodes {
        let region = crate::readonly::declaration(source, node, &memories)?;
        tables.name(&tables.list(node)?[1])?;
        if tables
            .readonly_regions
            .insert(region.name.clone(), readonly_regions.len())
            .is_some()
        {
            return Err(tables
                .error(node, "duplicate readonly region")
                .with_code("E_READONLY_REGION"));
        }
        readonly_regions.push(region);
    }
    for (node, _) in &function_nodes {
        let items = tables.list(node)?;
        let name = tables.name(
            items
                .get(1)
                .ok_or_else(|| tables.error(node, "missing function name"))?,
        )?;
        let next = tables.functions.len();
        tables.function_sites.insert(
            name.clone(),
            FunctionSite {
                name: items[1].span,
                result: items[2..]
                    .iter()
                    .find(|n| n.head() == Some("result"))
                    .map(|n| n.span),
            },
        );
        if tables.functions.insert(name, next).is_some() {
            return Err(tables.error(node, "duplicate function"));
        }
    }
    for node in &type_nodes {
        if let Some(parts) = node.list()
            && let Some(definition) = parts.get(2)
            && definition.head() == Some("newtype")
        {
            let fields = tables.list(definition)?;
            tables.count(definition, fields, 2)?;
            let scalar = fields[1]
                .atom()
                .and_then(NumType::parse)
                .filter(|n| *n != NumType::V128)
                .ok_or_else(|| tables.error(definition, "newtype requires i32, i64, f32 or f64"))?;
            tables
                .newtypes
                .insert(TypeId(tables.types[tables.atom(&parts[1])?]), scalar);
        }
    }
    tables.collect_values(root)?;
    for site in bounds {
        tables.collect_values(&site.clause)?;
    }
    let mut types = Vec::new();
    for node in type_nodes {
        let items = tables.list(node)?;
        let name = tables.name(&items[1])?;
        let kind = if node.head() == Some("closure") {
            let mut captures = Vec::new();
            for capture in items[2..].iter().filter(|n| n.head() == Some("capture")) {
                let parts = tables.list(capture)?;
                tables.count(capture, parts, 3)?;
                captures.push(Field {
                    name: Some(tables.name(&parts[1])?),
                    ty: tables.ty(&parts[2])?,
                    mutable: false,
                    packed: None,
                });
            }
            TypeKind::Closure {
                captures,
                function: FunctionId(tables.functions[&name]),
            }
        } else {
            tables.count(node, items, 3)?;
            let definition = tables.list(&items[2])?;
            match items[2].head() {
                Some("newtype") => TypeKind::Newtype(tables.newtypes[&TypeId(tables.types[&name])]),
                Some("resource") => {
                    tables.count(&items[2], definition, 3)?;
                    let kind = definition[1]
                        .text()
                        .ok_or_else(|| tables.error(node, "resource requires a kind string"))?
                        .to_owned();
                    let name = tables.name(&definition[2])?;
                    let close = FunctionId(
                        *tables
                            .functions
                            .get(&name)
                            .ok_or_else(|| tables.error(node, "unknown resource destructor"))?,
                    );
                    TypeKind::Resource { kind, close }
                }
                Some("struct") => {
                    let mut fields = Vec::new();
                    let mut names = BTreeSet::new();
                    for field in &definition[1..] {
                        let parts = tables.list(field)?;
                        if field.head() != Some("field") || !(2..=3).contains(&parts.len()) {
                            return Err(tables.error(field, "expected (field [$name] type)"));
                        }
                        let (name, storage) = if parts.len() == 3 {
                            let name = tables.name(&parts[1])?;
                            if !names.insert(name.clone()) {
                                return Err(tables.error(field, "duplicate field"));
                            }
                            (Some(name), &parts[2])
                        } else {
                            (None, &parts[1])
                        };
                        fields.push(tables.field(name, storage)?);
                    }
                    TypeKind::Struct(fields)
                }
                Some("array") => {
                    tables.count(&items[2], definition, 2)?;
                    let node = &definition[1];
                    let element = if node.head() == Some("mut") {
                        tables
                            .list(node)?
                            .get(1)
                            .ok_or_else(|| tables.error(node, "missing array storage type"))?
                    } else {
                        node
                    };
                    let packed = match element.atom() {
                        Some("i8") => Some(Packed::I8),
                        Some("i16") => Some(Packed::I16),
                        _ => None,
                    };
                    TypeKind::Array(if packed.is_some() {
                        if node.head() == Some("mut") {
                            tables.count(node, tables.list(node)?, 2)?;
                        }
                        Field {
                            name: None,
                            ty: Type::Num(NumType::I32),
                            mutable: node.head() == Some("mut"),
                            packed,
                        }
                    } else {
                        tables.field(None, node)?
                    })
                }
                Some("variant") => {
                    if definition.len() == 1 {
                        return Err(tables.error(node, "expected at least one variant"));
                    }
                    let mut variants = Vec::new();
                    let mut names = BTreeSet::new();
                    for case in &definition[1..] {
                        let parts = tables.list(case)?;
                        let name =
                            tables.variant_name(parts.first().ok_or_else(|| {
                                tables.error(case, "missing variant identifier")
                            })?)?;
                        if !names.insert(name.clone()) {
                            return Err(tables.error(case, "duplicate variant"));
                        }
                        let payloads = parts[1..]
                            .iter()
                            .map(|n| tables.ty(n))
                            .collect::<Result<_, _>>()?;
                        variants.push(Variant { name, payloads });
                    }
                    TypeKind::Variant(variants)
                }
                _ => return Err(tables.error(node, "expected GC struct, array, or variant type")),
            }
        };
        types.push(TypeDecl {
            name,
            kind,
            abi: None,
            abi_facts: None,
        });
    }
    types.extend(tables.values.borrow().types.iter().cloned());
    let mut functions = Vec::new();
    let mut tags = Vec::new();
    for node in tag_nodes {
        let items = tables.list(node)?;
        let mut payloads = Vec::new();
        for item in &items[2..] {
            if item.head() != Some("param") {
                return Err(tables.error(item, "expected (param tag payload types...)"));
            }
            for payload in &tables.list(item)?[1..] {
                payloads.push(tables.ty(payload)?);
            }
        }
        tags.push(TagDecl {
            name: tables.name(&items[1])?,
            payloads,
        });
    }
    let mut bodies = Vec::new();
    let mut exported_names = BTreeSet::new();
    for (node, import) in function_nodes {
        let items = tables.list(node)?;
        let name = tables.name(&items[1])?;
        let closure = node.head() == Some("closure");
        let kind = if let Some((module, name)) = import {
            FunctionKind::Import { module, name }
        } else if closure {
            FunctionKind::Closure {
                environment: TypeId(tables.types[&name]),
            }
        } else {
            FunctionKind::Defined
        };
        let mut function = Function {
            name,
            kind,
            locals: vec![],
            params: vec![],
            captures: vec![],
            result: Type::Unit,
            exports: vec![],
            body: None,
            import_contract: None,
            effect_contract: None,
        };
        let mut body = None;
        let mut result_seen = false;
        let mut names = BTreeSet::new();
        for item in &items[2..] {
            match item.head() {
                Some("param" | "capture") if body.is_none() => {
                    let capture = item.head() == Some("capture");
                    if capture && !closure {
                        return Err(
                            tables.error(item, "capture is only valid in a closure declaration")
                        );
                    }
                    let parts = tables.list(item)?;
                    tables.count(item, parts, 3)?;
                    let name = tables.name(&parts[1])?;
                    if !names.insert(name.clone()) {
                        return Err(tables.error(item, "duplicate binding"));
                    }
                    let id = LocalId(function.locals.len());
                    let kind = if capture {
                        BindingKind::Capture(function.captures.len())
                    } else {
                        BindingKind::Parameter
                    };
                    function.locals.push(Binding {
                        name,
                        span: parts[1].span,
                        ty: tables.ty(&parts[2])?,
                        kind,
                    });
                    if capture {
                        function.captures.push(id);
                    } else {
                        function.params.push(id);
                    }
                }
                Some("result") if body.is_none() => {
                    let parts = tables.list(item)?;
                    tables.count(item, parts, 2)?;
                    if result_seen {
                        return Err(tables.error(item, "duplicate result declaration"));
                    }
                    result_seen = true;
                    function.result = tables.ty(&parts[1])?;
                }
                Some("effect") if body.is_none() => {
                    if matches!(function.kind, FunctionKind::Import { .. }) {
                        return Err(tables.error(item, "imports cannot declare effect contracts"));
                    }
                    if function.effect_contract.is_some() {
                        return Err(tables.error(item, "duplicate effect contract"));
                    }
                    function.effect_contract = Some(tables.effect_contract(item)?);
                }
                Some("export") if body.is_none() => {
                    let parts = tables.list(item)?;
                    tables.count(item, parts, 2)?;
                    let name = parts[1]
                        .text()
                        .ok_or_else(|| tables.error(item, "expected export string"))?
                        .to_owned();
                    if closure || matches!(function.kind, FunctionKind::Import { .. }) {
                        return Err(tables.error(item, "exports belong to defined functions"));
                    }
                    if !exported_names.insert(name.clone()) {
                        return Err(tables.error(item, "duplicate export"));
                    }
                    function.exports.push(name);
                }
                _ => {
                    if body.is_some() {
                        return Err(tables.error(item, "expected one expression as function body"));
                    }
                    if item.list().is_none()
                        && item.atom().is_none()
                        && !matches!(
                            item.kind,
                            crate::sexpr::Kind::Text(_) | crate::sexpr::Kind::Bytes(_)
                        )
                    {
                        return Err(tables.error(item, "expected folded expression"));
                    }
                    body = Some(item);
                }
            }
        }
        if matches!(function.kind, FunctionKind::Import { .. }) && body.is_some() {
            return Err(tables.error(node, "import cannot have a body"));
        }
        functions.push(function);
        bodies.push(body);
    }
    let mut tests = Vec::new();
    for (index, node) in test_nodes.into_iter().enumerate() {
        let items = tables.list(node)?;
        let base = format!("__walz_test_{index}");
        let mut export = base.clone();
        let mut suffix = 0;
        while !exported_names.insert(export.clone()) {
            suffix += 1;
            export = format!("{base}_{suffix}");
        }
        tests.push(TestDecl {
            name: items[1].text().unwrap().to_owned(),
            span: node.span,
            function: FunctionId(functions.len()),
            export: export.clone(),
        });
        functions.push(Function {
            name: format!("$__test{index}"),
            kind: FunctionKind::Defined,
            locals: vec![],
            params: vec![],
            captures: vec![],
            result: Type::Num(NumType::I32),
            exports: vec![export],
            body: None,
            import_contract: None,
            effect_contract: None,
        });
        bodies.push(Some(&items[2]));
    }
    let mut benches = Vec::new();
    for (index, node) in bench_nodes.into_iter().enumerate() {
        let items = tables.list(node)?;
        let mut cursor = 2;
        let mut locals = Vec::new();
        let mut params = Vec::new();
        if let Some(parameter) = items.get(cursor).filter(|n| n.head() == Some("param")) {
            let parts = tables.list(parameter)?;
            tables.count(parameter, parts, 3)?;
            if tables.ty(&parts[2])? != Type::Num(NumType::I32) {
                return Err(tables.error(parameter, "expected an i32 iteration parameter"));
            }
            locals.push(Binding {
                name: tables.name(&parts[1])?,
                span: parts[1].span,
                ty: Type::Num(NumType::I32),
                kind: BindingKind::Parameter,
            });
            params.push(LocalId(0));
            cursor += 1;
        }
        if items.get(cursor).is_some_and(|n| n.head() == Some("param")) {
            return Err(tables.error(node, "benchmarks accept at most one iteration parameter"));
        }
        let result = if let Some(result) = items.get(cursor).filter(|n| n.head() == Some("result"))
        {
            let parts = tables.list(result)?;
            tables.count(result, parts, 2)?;
            cursor += 1;
            tables.ty(&parts[1])?
        } else {
            Type::Unit
        };
        if items.len() != cursor + 1 {
            return Err(tables.error(node, "expected one benchmark body after optional (param $iteration i32) and (result type)"));
        }
        let base = format!("__walz_bench_{index}");
        let mut export = base.clone();
        let mut suffix = 0;
        while !exported_names.insert(export.clone()) {
            suffix += 1;
            export = format!("{base}_{suffix}");
        }
        benches.push(BenchDecl {
            name: items[1].text().unwrap().to_owned(),
            span: node.span,
            function: FunctionId(functions.len()),
            export: export.clone(),
        });
        functions.push(Function {
            name: format!("$__bench{index}"),
            kind: FunctionKind::Defined,
            locals,
            params,
            captures: vec![],
            result,
            exports: vec![export],
            body: None,
            import_contract: None,
            effect_contract: None,
        });
        bodies.push(Some(&items[cursor]));
    }
    let mut laws = Vec::new();
    for (index, node) in law_nodes.into_iter().enumerate() {
        let items = tables.list(node)?;
        let mut locals = Vec::new();
        let mut params = Vec::new();
        let mut names = BTreeSet::new();
        for parameter in &items[2..items.len() - 1] {
            let parts = tables.list(parameter)?;
            tables.count(parameter, parts, 3)?;
            if parameter.head() != Some("param") {
                return Err(tables.error(parameter, "expected law parameter"));
            }
            let name = tables.name(&parts[1])?;
            if !names.insert(name.clone()) {
                return Err(tables.error(parameter, "duplicate binding"));
            }
            params.push(LocalId(locals.len()));
            locals.push(Binding {
                name,
                span: parts[1].span,
                ty: tables.ty(&parts[2])?,
                kind: BindingKind::Parameter,
            });
        }
        laws.push(LawDecl {
            name: items[1].text().unwrap().into(),
            span: node.span,
            function: FunctionId(functions.len()),
        });
        functions.push(Function {
            name: format!("$__law{index}"),
            kind: FunctionKind::Defined,
            locals,
            params,
            captures: vec![],
            result: Type::Num(NumType::I32),
            exports: vec![],
            body: None,
            import_contract: None,
            effect_contract: Some(crate::effects::EffectContract {
                span: node.span,
                allowed: crate::effects::EffectSet::default(),
            }),
        });
        bodies.push(items.last());
    }
    let mut module = Module {
        abi_suspends: Default::default(),
        memories,
        atomic_cells,
        readonly_regions,
        components,
        race_free: race_mode.is_some(),
        race_span: race_mode.unwrap_or(root.span),
        types,
        functions,
        tests,
        benches,
        laws,
        tags,
    };
    crate::interface::apply(source, &mut module, &abi_nodes)?;
    let constraints = crate::bounds::resolve(
        source,
        bounds,
        |node| {
            Ok(if node.atom().is_some_and(|n| n.starts_with('$')) {
                crate::bounds::Argument::Heap(tables.type_id(node)?)
            } else {
                crate::bounds::Argument::Value(tables.payload_ty(node)?)
            })
        },
        |node| tables.payload_ty(node),
        |node| tables.effect_contract(node),
    )?;
    let obligations = crate::bounds::check_shapes(source, &module, constraints)?;
    // Body checking sees signatures only: no dependency on declaration order.
    let headers = module.clone();
    for (index, body) in bodies.into_iter().enumerate() {
        if matches!(module.functions[index].kind, FunctionKind::Import { .. }) {
            continue;
        }
        let function = &headers.functions[index];
        if let Some(body) = body {
            macros.function_check(source, body)?;
        }

        let mut checker = Checker {
            tables: &tables,
            module: &headers,
            locals: function.locals.clone(),
            next_loop: 0,
            function_result: function.result.clone(),
            remaining_work: 1_000_000,
            probing: false,
            expression_depth: 0,
            closure: matches!(function.kind, FunctionKind::Closure { .. }),
            function_name: function.name.clone(),
        };
        let scope = checker
            .locals
            .iter()
            .enumerate()
            .map(|(id, b)| (b.name.clone(), LocalId(id)))
            .collect();
        let expression = if let Some(body) = body {
            let checked = checker.expr_as(body, &scope, &[], true, &function.result);
            checked.map_err(|error| {
                if let Some(site) = tables.function_sites.get(&function.name) {
                    error.expected_at(
                        source,
                        site.result.unwrap_or(site.name),
                        format!(
                            "result of `{}` declared as {}{}",
                            function.name,
                            tables.type_name(&function.result),
                            if site.result.is_none() {
                                " (omitted result means unit)"
                            } else {
                                ""
                            }
                        ),
                    )
                } else {
                    error
                }
            })?
        } else {
            Expr {
                ty: Type::Unit,
                // No expression exists in the source for an omitted unit body.
                span: Span {
                    start: root.span.start,
                    end: root.span.start,
                },
                kind: ExprKind::Nop,
            }
        };
        if !function.result.accepts(&expression.ty) {
            let site = tables.function_sites.get(&function.name);
            let mut error = Error::at(
                source,
                site.map(|site| site.name).unwrap_or(expression.span),
                format!(
                    "type mismatch: expected {}, got {}",
                    tables.type_name(&function.result),
                    tables.type_name(&expression.ty)
                ),
            )
            .with_code("E_TYPE_MISMATCH");
            if let Some(site) = site {
                error = error.expected_at(
                    source,
                    site.result.unwrap_or(site.name),
                    format!(
                        "result of `{}` declared as {}",
                        function.name,
                        tables.type_name(&function.result)
                    ),
                );
            }
            return Err(error);
        }
        module.functions[index].body = Some(expression);
        module.functions[index].locals = checker.locals;
        if let Some(body) = body {
            macros.finish_function(source, body)?;
        }
    }
    let mut export_names = module
        .functions
        .iter()
        .flat_map(|f| f.exports.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    for memory in &module.memories {
        for name in &memory.exports {
            if !export_names.insert(name.clone()) {
                return Err(tables
                    .error(root, "duplicate memory export")
                    .with_code("E_MEMORY_DECL"));
            }
        }
    }
    // Inference may compose value types that never appeared in annotations.
    module.types.truncate(tables.types.len());
    module
        .types
        .extend(tables.values.borrow().types.iter().cloned());
    Ok((module, obligations))
}

/// Check unused aliases without making their private storage part of emission.
pub(crate) fn validate_alias_targets(
    source: &str,
    module: &Module,
    targets: &[Sexp],
) -> Result<(), Error> {
    if targets.is_empty() {
        return Ok(());
    }
    let mut tables = Tables::new(source);
    for (index, declaration) in module.types.iter().enumerate() {
        match &declaration.kind {
            TypeKind::Newtype(n) => {
                tables.types.insert(declaration.name.clone(), index);
                tables.newtypes.insert(TypeId(index), *n);
            }
            TypeKind::Value(value) => {
                tables
                    .values
                    .get_mut()
                    .ids
                    .insert(value.clone(), TypeId(index));
                tables.values.get_mut().types.push(declaration.clone());
            }
            _ => {
                tables.types.insert(declaration.name.clone(), index);
            }
        }
    }
    for target in targets {
        tables.collect_values(target)?;
        tables.payload_ty(target)?;
    }
    let mut validation = module.clone();
    validation.types.truncate(tables.types.len());
    validation.types.extend(tables.values.into_inner().types);
    crate::ownership::validate(source, &validation)?;
    Ok(())
}

#[derive(Clone)]
struct LoopContext {
    name: String,
    id: LoopId,
    bindings: Vec<LocalId>,
    block: Option<Type>,
}

struct Checker<'a, 's> {
    tables: &'a Tables<'s>,
    module: &'a Module,
    locals: Vec<Binding>,
    next_loop: usize,
    function_result: Type,
    remaining_work: usize,
    probing: bool,
    expression_depth: usize,
    closure: bool,
    function_name: String,
}

impl Checker<'_, '_> {
    fn inferred_syntax(&self, node: &Sexp, ty: &Type) -> Result<crate::macros::Inferred, Error> {
        if *ty == Type::Never {
            return Err(self
                .tables
                .error(
                    node,
                    "cannot reify the bottom type; supply a value with a concrete type",
                )
                .with_code("E_MACRO_INFERENCE"));
        }
        let mut syntax = crate::sexpr::parse(&self.tables.type_name(ty))?;
        fn locate(n: &mut Sexp, span: Span) {
            n.span = span;
            if let crate::sexpr::Kind::List(p) = &mut n.kind {
                for c in p {
                    locate(c, span);
                }
            }
        }
        locate(&mut syntax, node.span);
        let classes = [
            "Add", "Sub", "Mul", "Div", "Eq", "Ord", "Logical", "Bitwise", "Shift",
        ]
        .into_iter()
        .filter(|n| crate::typeclasses::Class::parse(n).unwrap().supports(ty))
        .collect();
        Ok(crate::macros::Inferred {
            syntax,
            classes,
            info: self.type_info(ty),
        })
    }
    fn unknown_local(&self, node: &Sexp, name: &str, scope: &Scope) -> Error {
        let mut error = self
            .tables
            .error(node, format!("unbound local `{name}`"))
            .with_code("E_UNKNOWN_LOCAL");
        let candidates = scope
            .iter()
            .filter(|(candidate, _)| !candidate.starts_with("$__"))
            .filter_map(|(candidate, id)| {
                name_distance(name, candidate).map(|d| (d, candidate, id))
            })
            .collect::<Vec<_>>();
        if let Some(best) = candidates.iter().map(|(distance, _, _)| *distance).min() {
            let candidates = candidates
                .iter()
                .filter(|(d, _, _)| *d == best)
                .take(3)
                .collect::<Vec<_>>();
            error = error.help(format!(
                "did you mean {}?",
                candidates
                    .iter()
                    .map(|(_, name, _)| format!("`{name}`"))
                    .collect::<Vec<_>>()
                    .join(" or ")
            ));
            if candidates.len() == 1 {
                let (_, _, id) = candidates[0];
                let binding = &self.locals[id.0];
                error = error.related(
                    self.tables.source,
                    binding.span,
                    format!("`{}` is bound here", binding.name),
                );
            }
        } else {
            let available = scope
                .iter()
                .filter(|(name, _)| !name.starts_with("$__"))
                .take(5)
                .collect::<Vec<_>>();
            for (_, id) in available.iter().take(4) {
                let binding = &self.locals[id.0];
                error = error.related(
                    self.tables.source,
                    binding.span,
                    format!(
                        "available local `{}`: {}",
                        binding.name,
                        self.tables.type_name(&binding.ty)
                    ),
                );
            }
            if available.len() > 4 {
                error = error.note("additional available locals omitted");
            }
            if !available.is_empty() {
                error = error.help("if an existing binding was intended, use one of the available local names; otherwise introduce a binding with let before this use");
            }
            if self.closure {
                error = error.note("captures must be explicit: declare (capture $name Type) on this closure and provide its value in closure.new");
            }
        }
        if let Some(rule) = self
            .tables
            .macros
            .and_then(|m| m.caller_scope_binding(self.tables.source, node, name))
        {
            error = error.related(self.tables.source, rule, "bind makes the supplied names visible only to the declared body")
                .help("captured initializers use the caller's scope; introduce dependent values with an ordinary @let outside this macro call, or pass the dependency explicitly");
        }
        error
    }

    fn numeric_repair_hint(
        &self,
        error: Error,
        signature: &NumericSignature,
        actual: &Type,
        expected: Option<&Type>,
    ) -> Error {
        // Offer another operator only when the failed operand and surrounding
        // result agree on its type, and the alternative really exists. This is
        // a conditional editing hint, never an implicit numeric conversion.
        if let Some(Type::Num(result)) = expected
            && actual == &Type::Num(*result)
            && signature.output != *result
            && let Some((_, suffix)) = signature.name.split_once('.')
        {
            let candidate = format!("{}.{suffix}", result.wat());
            if NumericOp::parse(&candidate).is_some_and(|op| {
                let alternative = op.signature();
                alternative.input == *result
                    && alternative.output == *result
                    && alternative.arity == signature.arity
            }) {
                return error.note(format!("surrounding context expects {}", result.wat()))
                    .help(format!("if a {} operation is intended, use `{candidate}` and make every operand {} (including numeric constants); otherwise check the surrounding type requirement", result.wat(), result.wat()));
            }
        }
        error
    }

    fn is_owned(&self, ty: &Type) -> bool {
        match self.tables.value(ty) {
            Some(
                ValueType::Own(_)
                | ValueType::Future(_)
                | ValueType::Stream(_)
                | ValueType::Task(_),
            ) => true,
            Some(value) => value
                .variants(TypeId(0))
                .iter()
                .flat_map(|v| &v.payloads)
                .any(|ty| self.is_owned(ty)),
            None => crate::ownership::Kinds::new(self.module).is_owned(ty),
        }
    }

    fn expressions_as(
        &mut self,
        nodes: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
        expected: &[Type],
    ) -> Result<Vec<Expr>, Error> {
        nodes
            .iter()
            .zip(expected)
            .map(|(node, ty)| self.expr_as(node, scope, loops, false, ty))
            .collect()
    }

    fn payload_bindings(
        &mut self,
        node: &Sexp,
        payloads: &[Type],
        scope: &Scope,
    ) -> Result<(Scope, Vec<LocalId>), Error> {
        let binders = self.tables.list(node)?;
        if binders.len() != payloads.len() {
            return Err(self.tables.error(
                node,
                format!(
                    "payload binding count: expected {}, got {}",
                    payloads.len(),
                    binders.len()
                ),
            ));
        }
        let mut nested = scope.clone();
        let mut names = BTreeSet::new();
        let mut bindings = Vec::new();
        for (binder, ty) in binders.iter().zip(payloads) {
            let name = self.tables.name(binder)?;
            if !names.insert(name.clone()) {
                return Err(self.tables.error(binder, "duplicate payload binding"));
            }
            let id = self.local(name.clone(), binder.span, ty.clone(), BindingKind::Match);
            nested.insert(name, id);
            bindings.push(id);
        }
        Ok((nested, bindings))
    }

    fn pair_let(
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
        let Some(shape @ ValueType::Pair { .. }) = self.tables.value(&subject.ty) else {
            return Err(self
                .tables
                .error(&items[2], "pair pattern requires a pair value"));
        };
        let Type::Value(ty) = subject.ty else {
            unreachable!()
        };
        let payloads = &shape.variants(ty)[0].payloads;
        let (nested, bindings) = self.payload_bindings(&items[1], payloads, scope)?;
        let body = self.expr_expected(&items[3], &nested, loops, tail, expected)?;
        Ok(Expr {
            ty: body.ty.clone(),
            span: node.span,
            kind: ExprKind::Match {
                ty,
                subject: Box::new(subject),
                arms: vec![MatchArm {
                    variant: 0,
                    bindings,
                    body,
                }],
            },
        })
    }

    fn result_let(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
        expected: Option<&Type>,
    ) -> Result<(Type, ExprKind), Error> {
        self.tables.count(node, items, 4)?;
        let subject = self.expr(&items[2], scope, loops, false)?;
        let Some(ValueType::Result { ok, err }) = self.tables.value(&subject.ty) else {
            return Err(self
                .tables
                .error(&items[2], "result.let subject requires a result value"));
        };
        if self.is_owned(&err) {
            return Err(self.tables.error(
                node,
                "result.let error payload must be shared data in this version",
            ));
        }
        let payloads = if ok == Type::Unit { vec![] } else { vec![ok] };
        let (nested, bindings) = self.payload_bindings(&items[1], &payloads, scope)?;
        let contextual =
            expected.filter(|ty| matches!(self.tables.value(ty), Some(ValueType::Result { .. })));
        let body = if let Some(expected) = contextual {
            self.expr_as(&items[3], &nested, loops, tail, expected)?
        } else if items[3].head() == Some("result.ok") && items[3].list().unwrap().len() <= 2 {
            // The error type comes from the subject. Only this local success
            // payload is synthesized; unrelated bindings are never searched.
            let body_items = items[3].list().unwrap();
            let value = body_items
                .get(1)
                .map(|value| self.expr(value, &nested, loops, false))
                .transpose()?;
            let success = value.as_ref().map_or(Type::Unit, |value| value.ty.clone());
            if matches!(success, Type::Never | Type::Unit) && value.is_some() {
                return Err(self
                    .tables
                    .error(&items[3], "result success type needs a type annotation"));
            }
            let output = self.tables.intern_value(ValueType::Result {
                ok: success.clone(),
                err: err.clone(),
            });
            Expr {
                ty: Type::Value(output),
                span: items[3].span,
                kind: ExprKind::VariantNew {
                    ty: output,
                    variant: 0,
                    values: if success == Type::Unit {
                        vec![]
                    } else {
                        value.into_iter().collect()
                    },
                },
            }
        } else {
            self.expr(&items[3], &nested, loops, tail)?
        };
        let result = if body.ty == Type::Never {
            contextual.cloned().ok_or_else(|| {
                self.tables
                    .error(node, "result.let output needs a type annotation")
            })?
        } else {
            body.ty.clone()
        };
        self.finish_result_let(node, subject, bindings, body, err, result)
    }

    fn finish_result_let(
        &mut self,
        node: &Sexp,
        subject: Expr,
        bindings: Vec<LocalId>,
        body: Expr,
        err: Type,
        result: Type,
    ) -> Result<(Type, ExprKind), Error> {
        let Type::Value(subject_ty) = subject.ty else {
            unreachable!()
        };
        let Some(ValueType::Result { err: body_err, .. }) = self.tables.value(&result) else {
            return Err(self
                .tables
                .error(node, "result.let body needs a result type annotation"));
        };
        // Propagation keeps E invariant, including reference nullability.
        self.tables.expect_type(node, &err, &body_err)?;
        self.tables.expect_type(node, &body_err, &err)?;
        let Type::Value(output_ty) = result else {
            unreachable!()
        };
        let mut error_bindings = Vec::new();
        let mut values = Vec::new();
        if err != Type::Unit {
            let local = self.local(
                format!("$__result_error{}", self.locals.len()),
                node.span,
                err.clone(),
                BindingKind::Match,
            );
            error_bindings.push(local);
            values.push(Expr {
                ty: err.clone(),
                span: node.span,
                kind: ExprKind::Local(local),
            });
        }
        let error_body = Expr {
            ty: result.clone(),
            span: node.span,
            kind: ExprKind::VariantNew {
                ty: output_ty,
                variant: 1,
                values,
            },
        };
        Ok((
            result,
            ExprKind::Match {
                ty: subject_ty,
                subject: Box::new(subject),
                arms: vec![
                    MatchArm {
                        variant: 0,
                        bindings,
                        body,
                    },
                    MatchArm {
                        variant: 1,
                        bindings: error_bindings,
                        body: error_body,
                    },
                ],
            },
        ))
    }

    fn probe(
        &mut self,
        node: &Sexp,
        scope: &Scope,
        loops: &[LoopContext],
        expected: Option<&Type>,
    ) -> Result<Type, Error> {
        let locals = self.locals.len();
        let next_loop = self.next_loop;
        let probing = self.probing;
        // A probe must not commit a macro expansion against temporary lexical
        // IDs. Let the nearest non-probing expression retry with ordinary syntax.
        self.probing = true;
        let result = self
            .expr_expected(node, scope, loops, false, expected)
            .map(|e| e.ty);
        self.locals.truncate(locals);
        self.next_loop = next_loop;
        self.probing = probing;
        result
    }

    fn fold_result(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
        expected: Option<&Type>,
    ) -> Result<Expr, Error> {
        self.tables.count(node, items, 5)?;
        let source = self.probe(&items[1], scope, loops, None)?;
        let Some(ValueType::Stream(payload)) = self.tables.value(&source) else {
            return Err(self.tables.error(
                &items[1],
                "stream.fold-result source requires a stream of results",
            ));
        };
        let Some(ValueType::Result { ok: item, err }) = self.tables.value(&payload) else {
            return Err(self.tables.error(
                &items[1],
                "stream.fold-result source requires a stream of results",
            ));
        };
        let state_expected = expected
            .and_then(|ty| self.tables.value(ty))
            .and_then(|v| match v {
                ValueType::Result { ok, .. } => Some(ok),
                _ => None,
            });
        let initial = self.probe(&items[2], scope, loops, state_expected.as_ref())?;
        let state = state_expected.unwrap_or_else(|| initial.clone());
        self.tables.expect_type(&items[2], &state, &initial)?;
        if matches!(state, Type::Unit | Type::Never) || self.is_owned(&state) {
            return Err(self.tables.error(
                &items[2],
                "fold initial state must produce shared non-unit data; add a type annotation",
            ));
        }
        let result = Type::Value(self.tables.intern_value(ValueType::Result {
            ok: state.clone(),
            err: err.clone(),
        }));
        if let Some(expected) = expected {
            self.tables.expect_type(node, expected, &result)?;
        }
        let type_node = |ty: &Type| -> Result<Sexp, Error> {
            let mut syntax = crate::sexpr::parse(&self.tables.type_name(ty))?;
            syntax.span = node.span;
            Ok(syntax)
        };
        let mut explicit = node.clone();
        explicit.kind = crate::sexpr::Kind::List(vec![
            items[0].clone(),
            type_node(&item)?,
            type_node(&state)?,
            type_node(&err)?,
            items[1].clone(),
            items[2].clone(),
            items[3].clone(),
            items[4].clone(),
        ]);
        let expanded = crate::scoped::expand(self.tables.source, &explicit)?;
        self.tables.collect_values(&expanded)?;
        self.expr_as(&expanded, scope, loops, tail, &result)
    }

    fn local(&mut self, name: String, span: Span, ty: Type, kind: BindingKind) -> LocalId {
        let id = LocalId(self.locals.len());
        self.locals.push(Binding {
            name,
            span,
            ty,
            kind,
        });
        id
    }

    fn field(&self, node: &Sexp, ty: TypeId, field: &Sexp) -> Result<(usize, Field), Error> {
        let TypeKind::Struct(fields) = &self.module.types[ty.0].kind else {
            return Err(self.tables.error(node, "expected struct type"));
        };
        let name = self.tables.atom(field)?;
        let index = name
            .parse::<usize>()
            .ok()
            .or_else(|| fields.iter().position(|f| f.name.as_deref() == Some(name)));
        let index = index
            .filter(|&i| i < fields.len())
            .ok_or_else(|| self.tables.error(field, "unknown field"))?;
        Ok((index, fields[index].clone()))
    }

    fn array(&self, node: &Sexp, ty: TypeId) -> Result<Field, Error> {
        if let TypeKind::Array(field) = &self.module.types[ty.0].kind {
            Ok(field.clone())
        } else {
            Err(self.tables.error(node, "expected array type"))
        }
    }

    fn variants(&self, node: &Sexp, ty: TypeId) -> Result<Cow<'_, [Variant]>, Error> {
        if let Some(value) = self.tables.value(&Type::Value(ty)) {
            return match value {
                ValueType::List(_) => Err(self.tables.error(
                    node,
                    "list values require list operations; their representation cannot be matched",
                )),
                ValueType::Bytes
                | ValueType::Text
                | ValueType::Future(_)
                | ValueType::Stream(_)
                | ValueType::Task(_) => Err(self
                    .tables
                    .error(node, "opaque delivery endpoints require await/next/close")),
                value => Ok(Cow::Owned(value.variants(ty))),
            };
        }
        match &self.module.types[ty.0].kind {
            TypeKind::Variant(variants) => Ok(Cow::Borrowed(variants)),
            _ => Err(self.tables.error(node, "expected variant type")),
        }
    }

    fn variant(&self, node: &Sexp, ty: TypeId) -> Result<(usize, Variant), Error> {
        let name = self.tables.variant_name(node)?;
        self.variants(node, ty)?
            .iter()
            .enumerate()
            .find(|(_, variant)| variant.name == name)
            .map(|(index, variant)| (index, variant.clone()))
            .ok_or_else(|| self.tables.error(node, format!("unknown variant `{name}`")))
    }

    fn check(&self, node: &Sexp, expected: &Type, expression: &Expr) -> Result<(), Error> {
        self.tables.expect_type(node, expected, &expression.ty)
    }

    fn expr(
        &mut self,
        node: &Sexp,
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
    ) -> Result<Expr, Error> {
        self.expr_expected(node, scope, loops, tail, None)
    }

    fn expr_as(
        &mut self,
        node: &Sexp,
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
        expected: &Type,
    ) -> Result<Expr, Error> {
        let expression = self.expr_expected(node, scope, loops, tail, Some(expected))?;
        self.check(node, expected, &expression)?;
        Ok(expression)
    }

    fn local_value(
        &self,
        node: &Sexp,
        name: &str,
        scope: &Scope,
        moving: bool,
    ) -> Result<(Type, ExprKind), Error> {
        let id = *scope
            .get(name)
            .ok_or_else(|| self.unknown_local(node, name, scope))?;
        Ok((
            self.locals[id.0].ty.clone(),
            if moving {
                ExprKind::Move(id)
            } else {
                ExprKind::Local(id)
            },
        ))
    }

    fn numeric_constant(
        &self,
        node: &Sexp,
        ty: NumType,
        literal: &str,
    ) -> Result<(Type, ExprKind), Error> {
        let fragment = format!(
            "(module (func (result {}) ({}.const {})))",
            ty.wat(),
            ty.wat(),
            literal
        );
        wat::parse_str(&fragment).map_err(|_| {
            self.tables
                .error(node, "invalid numeric literal")
                .with_code("E_NUMERIC_LITERAL")
        })?;
        Ok((
            Type::Num(ty),
            ExprKind::Const {
                ty,
                literal: literal.to_owned(),
            },
        ))
    }

    fn operator_expr(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        operator: crate::typeclasses::Operator,
        scope: &Scope,
        loops: &[LoopContext],
        expected: Option<&Type>,
    ) -> Result<(Type, ExprKind), Error> {
        let tables = self.tables;
        let symbol = items[0].atom().unwrap();
        if items.len() != 3 {
            return Err(tables
                .error(
                    node,
                    format!("operator `{symbol}` requires exactly two operands"),
                )
                .with_code("E_OPERATOR_ARITY"));
        }
        // Comparisons return i32 independently of their operand type. Their
        // surrounding result must never choose the width of the operands.
        let mut input = operator.fixed_input().or_else(|| {
            expected
                .filter(|ty| operator.preserves_type() && !matches!(ty, Type::Unit | Type::Never))
                .cloned()
        });
        let mut args = [None, None];
        if input.is_none() {
            for (index, argument) in items[1..].iter().enumerate() {
                let local_count = self.locals.len();
                let next_loop = self.next_loop;
                match self.expr(argument, scope, loops, false) {
                    Ok(value) => {
                        if value.ty != Type::Never {
                            input = Some(value.ty.clone());
                        }
                        // Reuse successful elaboration: probing nested operators
                        // again would duplicate locals and exponential work.
                        args[index] = Some(value);
                        if input.is_some() {
                            break;
                        }
                    }
                    Err(error)
                        if matches!(
                            error.diagnostic().code,
                            "E_LITERAL_TYPE" | "E_OPERATOR_TYPE"
                        ) =>
                    {
                        // A contextual expression can allocate lexical IDs
                        // before reaching a literal that needs its input type.
                        self.locals.truncate(local_count);
                        self.next_loop = next_loop;
                        // Keep the work budget charged. Value-type interning is
                        // an idempotent cache, not a source binding or an effect.
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        let input = input.ok_or_else(|| tables.error(node,
            format!("cannot determine operand type for `{symbol}`"))
            .with_code("E_OPERATOR_TYPE")
            .help("add (type T) to the surrounding let, declare an arithmetic result type, or give one operand an explicit type such as (i64.const 1); comparisons do not infer operand types from their i32 result"))?;
        let op = operator.resolve(&input).ok_or_else(|| tables.error(node,
            format!("{} has no instance for {}", operator.class().name(), tables.type_name(&input)))
            .with_code("E_CLASS_INSTANCE")
            .help(format!("{} supports {}; use explicit Wasm instructions for other numeric semantics, and explicitly unwrap newtypes", operator.class().name(), operator.class().supported_types())))?;
        let mut values = Vec::new();
        for (index, (argument, cached)) in items[1..].iter().zip(args).enumerate() {
            let value = if let Some(value) = cached {
                self.check(argument, &input, &value).map(|()| value)
            } else {
                self.expr_as(argument, scope, loops, false, &input)
            }
            .map_err(|error| {
                error.expected_at(
                    tables.source,
                    items[0].span,
                    format!(
                        "operand {} of `{symbol}` requires {}",
                        index + 1,
                        tables.type_name(&input)
                    ),
                )
            })?;
            values.push(value);
        }
        let result = op.lower(values.try_into().unwrap(), node.span);
        Ok((result.ty, result.kind))
    }

    fn named_struct(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
    ) -> Result<(Type, ExprKind), Error> {
        use crate::sexpr::Kind;
        let type_node = items
            .get(1)
            .ok_or_else(|| self.tables.error(node, "missing constructor type"))?;
        let ty = self.tables.type_id(type_node)?;
        let TypeKind::Struct(fields) = &self.module.types[ty.0].kind else {
            return Err(self
                .tables
                .error(type_node, "named construction requires a struct type")
                .with_code("E_STRUCT_FIELDS"));
        };
        let fields = fields.clone();
        let plan = crate::construction::fields(self.tables.source, node, &fields, &items[2..])?;
        let mut values = vec![None; fields.len()];
        let mut bindings = Vec::new();
        for (index, initializer) in plan {
            let field = &fields[index];
            // Text elision is local to named fields. Numeric literals and local
            // reads use the same expected-type rules as every other expression.
            let expanded;
            let expression = if matches!(initializer.kind, Kind::Text(_) | Kind::Bytes(_)) {
                if !matches!(self.tables.value(&field.ty), Some(ValueType::Text)) {
                    return Err(self
                        .tables
                        .error(
                            initializer,
                            format!(
                                "literal is not valid for field `{}`; expected {}",
                                field.name.as_deref().unwrap_or(""),
                                self.tables.type_name(&field.ty)
                            ),
                        )
                        .with_code("E_STRUCT_LITERAL")
                        .help("use an explicit expression for bytes and other non-text values"));
                }
                expanded = Sexp {
                    context: Default::default(),
                    span: initializer.span,
                    kind: Kind::List(vec![
                        Sexp {
                            context: Default::default(),
                            span: initializer.span,
                            kind: Kind::Atom("text.lit".into()),
                        },
                        initializer.clone(),
                    ]),
                };
                &expanded
            } else {
                initializer
            };
            let value = self.expr_as(expression, scope, loops, false, &field.ty)?;
            // LocalId, rather than a source-level binding, makes these temporaries
            // hygienic. Initializers cannot refer to another field's temporary.
            let local = self.local(
                format!("$__struct.{}", self.locals.len()),
                initializer.span,
                field.ty.clone(),
                BindingKind::Let,
            );
            values[index] = Some(Expr {
                ty: field.ty.clone(),
                span: initializer.span,
                kind: ExprKind::Local(local),
            });
            bindings.push((local, value));
        }
        let mut body = Expr {
            ty: Type::Ref {
                ty,
                nullable: false,
            },
            span: node.span,
            kind: ExprKind::StructNew {
                ty,
                values: values.into_iter().map(Option::unwrap).collect(),
            },
        };
        // Evaluate in source order; only the final local reads use layout order.
        // Abrupt completion never reaches later initializers or allocation.
        for (local, value) in bindings.into_iter().rev() {
            body = Expr {
                ty: body.ty.clone(),
                span: node.span,
                kind: ExprKind::Let {
                    local,
                    value: Box::new(value),
                    body: Box::new(body),
                },
            };
        }
        Ok((body.ty, body.kind))
    }

    fn expr_expected_once(
        &mut self,
        node: &Sexp,
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
        expected: Option<&Type>,
    ) -> Result<Expr, Error> {
        self.remaining_work = self.remaining_work.checked_sub(1).ok_or_else(|| {
            self.tables
                .error(node, "local type elaboration limit exceeded")
        })?;
        if node.head() == Some("call")
            && let Some(callee) = node.list().and_then(|p| p.get(1))
            && let Some(call) = callee
                .atom()
                .and_then(|name| self.tables.inference?.call(name))
        {
            return self.infer_call(node, call, scope, loops, tail, expected);
        }
        if node.head().is_some_and(|n| n.starts_with("@$")) {
            let environment = self.tables.macros.expect("checker macro environment");
            let context = expected
                .map(|ty| self.inferred_syntax(node, ty))
                .transpose()?;
            environment.request(
                self.tables.source,
                node,
                context.as_ref(),
                |value, hint, projection| {
                    let ty = self.probe(value, scope, loops, if hint { expected } else { None })?;
                    self.project_type(value, &ty, projection)
                },
            )?;
            unreachable!("expansion restarts checking with ordinary syntax")
        }
        if matches!(
            node.kind,
            crate::sexpr::Kind::Text(_) | crate::sexpr::Kind::Bytes(_)
        ) {
            let ty = Type::Value(self.tables.intern_value(ValueType::Text));
            if expected != Some(&ty) {
                return Err(self.tables.error(node,"bare string literal requires an expected $string type")
                    .with_code("E_LITERAL_TYPE")
                    .help("add (type $string) or write an explicit (@string.lit \"...\"); strings are not converted to other types"));
            }
            let text = node.text().ok_or_else(|| {
                self.tables
                    .error(node, "string literal must contain valid UTF-8")
                    .with_code("E_LITERAL_TYPE")
                    .help("use @bytes.lit for arbitrary bytes")
            })?;
            return Ok(Expr {
                ty,
                span: node.span,
                kind: ExprKind::Data(text.as_bytes().to_vec()),
            });
        }
        if let Some(atom) = node.atom() {
            let mut offset = 0;
            let numeric = wast::lexer::Lexer::new(atom)
                .parse(&mut offset)
                .ok()
                .flatten()
                .is_some_and(|token| {
                    matches!(
                        token.kind,
                        wast::lexer::TokenKind::Integer(_) | wast::lexer::TokenKind::Float(_)
                    )
                });
            if !atom.starts_with('$') && !numeric {
                // Keep native stack keywords on the existing opaque macro
                // path. Only identifiers and numeric tokens are sugar here.
                return Err(self.tables.error(node, "expected folded expression"));
            }
            let (ty, kind) = if atom.starts_with('$') {
                self.local_value(node, atom, scope, false)?
            } else if let Some(Type::Num(ty)) = expected
                && *ty != NumType::V128
            {
                self.numeric_constant(node, *ty, atom)?
            } else {
                return Err(self.tables.error(node,
                    "bare literal requires an expected numeric type (i32, i64, f32 or f64)")
                    .with_code("E_LITERAL_TYPE")
                    .help("add a type annotation or write an explicit constant such as (i32.const 1); numeric widths are not inferred from the value"));
            };
            return Ok(Expr {
                ty,
                span: node.span,
                kind,
            });
        }
        if let Some(operator) = node.head().and_then(crate::typeclasses::Operator::parse) {
            return self
                .operator_expr(
                    node,
                    self.tables.list(node)?,
                    operator,
                    scope,
                    loops,
                    expected,
                )
                .map(|(ty, kind)| Expr {
                    ty,
                    kind,
                    span: node.span,
                });
        }
        match node.head() {
            Some("let") => return self.let_expr(node, scope, loops, tail, expected),
            Some("block") => return self.block_expr(node, scope, loops, tail, expected),
            Some("if") => return self.if_expr(node, scope, loops, tail, expected),
            _ => {}
        }
        // Recursive operator elaboration does not need a stack frame containing
        // the entire instruction dispatcher, especially in debug builds.
        self.folded_expr_expected(node, scope, loops, tail, expected)
    }

    fn memory_expr(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        op: &crate::threads::MemoryOp,
        scope: &Scope,
        loops: &[LoopContext],
    ) -> Result<(Type, ExprKind), Error> {
        let atomic_error = |message: &str| self.tables.error(node, message).with_code("E_ATOMIC");
        let selector = crate::threads::selector(items);
        let mut memory = if op.fence() {
            None
        } else {
            let id = if let Some(index) = selector {
                let name = self.tables.atom(&items[index])?;
                self.tables
                    .memories
                    .get(name)
                    .copied()
                    .or_else(|| crate::threads::index(name).map(|n| n as usize))
            } else {
                Some(0)
            };
            Some(MemoryId(
                id.filter(|i| *i < self.module.memories.len())
                    .ok_or_else(|| {
                        self.tables
                            .error(node, "unknown or missing linear memory")
                            .with_code("E_MEMORY_USE")
                    })?,
            ))
        };
        let mut start = 1 + usize::from(selector.is_some());
        let mut offset = None;
        let mut align = None;
        while let Some(part) = items.get(start).filter(|n| crate::threads::immediate(n)) {
            let token = part.atom().unwrap();
            let (kind, value) = token.split_once('=').unwrap();
            let value = crate::threads::index(value).ok_or_else(|| {
                atomic_error("memory immediate must be an unsigned memory32 integer")
            })?;
            let slot = if kind == "offset" {
                &mut offset
            } else {
                &mut align
            };
            if slot.replace(value).is_some() {
                return Err(atomic_error("duplicate memory immediate"));
            }
            start += 1;
        }
        if op.bytes() == 0 && (offset.is_some() || align.is_some()) {
            return Err(atomic_error(
                "this instruction has no offset or alignment immediate",
            ));
        }
        if align.is_some_and(|a| a != op.bytes()) {
            return Err(atomic_error(
                "atomic alignment must equal the natural access width in bytes",
            ));
        }
        if items.len() != start + op.inputs().len() {
            return Err(atomic_error(
                "atomic/memory operand count does not match its Wasm signature",
            ));
        }
        let cell = if let Some(address) = items.get(start).filter(|n| n.head() == Some("atomic.at"))
        {
            let fields = self.tables.list(address)?;
            if fields.len() != 2 {
                return Err(self
                    .tables
                    .error(address, "expected (@atomic.at $cell)")
                    .with_code("E_ATOMIC_CELL"));
            }
            let name = self.tables.atom(&fields[1])?;
            let id = AtomicCellId(*self.tables.atomic_cells.get(name).ok_or_else(|| {
                self.tables
                    .error(address, "unknown atomic cell")
                    .with_code("E_ATOMIC_CELL")
            })?);
            let declaration = &self.module.atomic_cells[id.0];
            if selector.is_none() {
                memory = Some(declaration.memory);
            }
            if memory != Some(declaration.memory)
                || offset.unwrap_or(0) != 0
                || !crate::race::compatible(op, declaration)
            {
                return Err(self.tables.error(node, "atomic operation must use the cell's memory, full width, and zero extra offset").with_code("E_ATOMIC_CELL"));
            }
            Some(id)
        } else {
            None
        };
        let mut args = Vec::new();
        for (index, (part, ty)) in items[start..].iter().zip(op.inputs()).enumerate() {
            if index == 0
                && let Some(id) = cell
            {
                args.push(Expr {
                    ty: Type::Num(NumType::I32),
                    span: part.span,
                    kind: ExprKind::Const {
                        ty: NumType::I32,
                        literal: self.module.atomic_cells[id.0].offset.to_string(),
                    },
                });
                continue;
            }
            args.push(self.expr_as(part, scope, loops, false, &Type::Num(*ty))?);
        }
        Ok((
            op.output().map(Type::Num).unwrap_or(Type::Unit),
            ExprKind::Memory {
                op: op.clone(),
                memory,
                cell,
                offset: offset.unwrap_or(0),
                args,
            },
        ))
    }

    fn folded_expr_expected(
        &mut self,
        node: &Sexp,
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
        expected: Option<&Type>,
    ) -> Result<Expr, Error> {
        use ExprKind::*;
        let items = self.tables.list(node)?;
        let head = items
            .first()
            .and_then(Sexp::atom)
            .ok_or_else(|| self.tables.error(node, "expected expression operator"))?;
        let tables = self.tables;
        let count = |n| tables.count(node, items, n);
        if head == "atomic.at" {
            return Err(tables
                .error(
                    node,
                    "an atomic cell reference is only allowed as an atomic address operand",
                )
                .with_code("E_ATOMIC_CELL"));
        }
        let (ty, kind) = if head == "readonly.load" {
            count(4)?;
            let id = tables
                .atom(&items[1])
                .ok()
                .and_then(|name| tables.readonly_regions.get(name))
                .copied()
                .ok_or_else(|| {
                    tables
                        .error(&items[1], "unknown readonly region")
                        .with_code("E_READONLY_REGION")
                })?;
            let (opcode, ty, width) = items[2]
                .atom()
                .and_then(crate::readonly::opcode)
                .ok_or_else(|| {
                    tables
                        .error(&items[2], "readonly.load supports u8, i32, i64 or v128")
                        .with_code("E_READONLY_LOAD")
                })?;
            let offset = self.expr_as(&items[3], scope, loops, false, &Type::Num(NumType::I32))?;
            (
                Type::Num(ty),
                Wasm {
                    instruction: crate::wasm::Instruction {
                        opcode: opcode.into(),
                        memories: vec![self.module.readonly_regions[id].memory],
                        immediates: vec![],
                        readonly: Some(crate::readonly::Load { region: id, width }),
                    },
                    args: vec![offset],
                },
            )
        } else if let Some(op) = crate::threads::lookup(head) {
            self.memory_expr(node, items, op, scope, loops)?
        } else if crate::wasm::supported(head) {
            let mut instruction = crate::wasm::Instruction {
                opcode: head.into(),
                memories: Vec::new(),
                immediates: Vec::new(),
                readonly: None,
            };
            let mut args = Vec::new();
            let mut lane = false;
            for item in &items[1..] {
                if args.is_empty()
                    && let Some(name) = item.atom()
                {
                    if let Some(id) = self.module.memories.iter().position(|m| m.name == name) {
                        instruction.memories.push(MemoryId(id));
                        continue;
                    }
                    if name.starts_with("offset=")
                        || name.starts_with("align=")
                        || (head.ends_with("_lane")
                            && !lane
                            && crate::threads::index(name).is_some())
                    {
                        instruction.immediates.push(name.into());
                        if head.ends_with("_lane") && !name.contains('=') {
                            lane = true;
                        }
                        continue;
                    }
                }
                let hint = Type::Num(
                    instruction.hint(
                        args.iter()
                            .filter(|e: &&Expr| matches!(e.ty, Type::Num(_)))
                            .count(),
                    ),
                );
                let value = if item.atom().is_some_and(|s| !s.starts_with('$')) {
                    self.expr_as(item, scope, loops, false, &hint)?
                } else {
                    self.expr(item, scope, loops, false)?
                };
                args.push(value);
            }
            let count = if head == "memory.copy" { 2 } else { 1 };
            if instruction.memories.is_empty() && !self.module.memories.is_empty() {
                instruction.memories = vec![MemoryId(0); count];
            }
            if instruction.memories.len() != count {
                return Err(tables
                    .error(node, "expected named memory operands")
                    .with_code("E_WAT_TYPE"));
            }
            let inputs = args.iter().map(|e| e.ty.clone()).collect::<Vec<_>>();
            let ty = instruction
                .result(&self.module.memories, &inputs)
                .map_err(|e| tables.error(node, e.to_string()).with_code("E_WAT_TYPE"))?;
            (ty, Wasm { instruction, args })
        } else if let Some(op) = DeliveryOp::parse(head) {
            self.delivery_expr(node, items, op, scope, loops)?
        } else if let Some(signature) = crate::simd::lookup(head) {
            let start = 1 + signature.immediate_count();
            count(start + signature.inputs().len())?;
            let immediates = items[1..start]
                .iter()
                .map(|n| self.tables.atom(n).map(str::to_owned))
                .collect::<Result<Vec<_>, _>>()?;
            let op = signature
                .build(immediates)
                .map_err(|e| tables.error(node, e).with_code("E_SIMD"))?;
            let mut args = Vec::new();
            for (index, (argument, ty)) in items[start..].iter().zip(op.inputs()).enumerate() {
                let value = self
                    .expr_as(argument, scope, loops, false, &Type::Num(*ty))
                    .map_err(|e| {
                        e.expected_at(
                            tables.source,
                            items[0].span,
                            format!("operand {} of `{head}` requires {}", index + 1, ty.wat()),
                        )
                    })?;
                args.push(value);
            }
            (Type::Num(op.output()), Simd { op, args })
        } else if head == "v128.const" {
            let literal = items[1..]
                .iter()
                .map(|n| self.tables.atom(n))
                .collect::<Result<Vec<_>, _>>()?
                .join(" ");
            crate::simd::constant(&literal)
                .map_err(|e| tables.error(node, e).with_code("E_SIMD"))?;
            (
                Type::Num(NumType::V128),
                Const {
                    ty: NumType::V128,
                    literal,
                },
            )
        } else if let Some(op) = NumericOp::parse(head) {
            let signature = op.signature();
            count(signature.arity + 1)?;
            let args = items[1..]
                .iter()
                .map(|node| {
                    self.expr_expected(node, scope, loops, false, Some(&Type::Num(signature.input)))
                })
                .collect::<Result<Vec<_>, _>>()?;
            for (index, (node, argument)) in items[1..].iter().zip(&args).enumerate() {
                self.check(node, &Type::Num(signature.input), argument)
                    .map_err(|error| {
                        let error = error.expected_at(
                            tables.source,
                            items[0].span,
                            format!(
                                "operand {} of `{head}` requires {}",
                                index + 1,
                                signature.input.wat()
                            ),
                        );
                        self.numeric_repair_hint(error, &signature, &argument.ty, expected)
                    })?;
            }
            (Type::Num(signature.output), Numeric { op, args })
        } else if let Some(ty) = head.strip_suffix(".const").and_then(NumType::parse) {
            count(2)?;
            self.numeric_constant(&items[1], ty, self.tables.atom(&items[1])?)?
        } else {
            match head {
                "local.get" | "move" => {
                    count(2)?;
                    let name = self.tables.atom(&items[1])?;
                    self.local_value(&items[1], name, scope, head == "move")?
                }
                "resource.new" => {
                    tables.count(node, items, 3)?;
                    let ty = tables.type_id(&items[1])?;
                    if !matches!(self.module.types[ty.0].kind, TypeKind::Resource { .. }) {
                        return Err(tables.error(node, "expected resource type"));
                    }
                    let handle = self.expr_expected(
                        &items[2],
                        scope,
                        loops,
                        false,
                        Some(&Type::Num(NumType::I32)),
                    )?;
                    (
                        Type::Resource(ty),
                        ResourceNew {
                            ty,
                            handle: Box::new(handle),
                        },
                    )
                }
                "task.spawn" => {
                    tables.count(node, items, 4)?;
                    let payload = tables.payload_ty(&items[1])?;
                    let name = tables.atom(&items[2])?;
                    let function = FunctionId(
                        *tables
                            .functions
                            .get(name)
                            .ok_or_else(|| tables.error(node, "unknown task closure"))?,
                    );
                    let f = &self.module.functions[function.0];
                    let FunctionKind::Closure { environment } = f.kind else {
                        return Err(tables.error(node, "task.spawn requires a declared closure"));
                    };
                    if !f.params.is_empty() || f.result != payload {
                        return Err(tables.error(
                            node,
                            "task closure must take no arguments and return the declared payload",
                        ));
                    }
                    let environment = self.expr_as(
                        &items[3],
                        scope,
                        loops,
                        false,
                        &Type::Ref {
                            ty: environment,
                            nullable: false,
                        },
                    )?;
                    (
                        Type::Value(tables.intern_value(ValueType::Task(payload))),
                        TaskSpawn {
                            function,
                            environment: Box::new(environment),
                        },
                    )
                }
                "task.join" | "task.drop" | "task.cancel" | "task.join-timeout" => {
                    let timed = head == "task.join-timeout";
                    tables.count(node, items, if timed { 4 } else { 3 })?;
                    let payload = tables.payload_ty(&items[1])?;
                    let ty = Type::Value(tables.intern_value(ValueType::Task(payload.clone())));
                    let timeout = if timed {
                        Some(Box::new(self.expr_as(
                            &items[2],
                            scope,
                            loops,
                            false,
                            &Type::Num(NumType::I32),
                        )?))
                    } else {
                        None
                    };
                    let task =
                        self.expr_as(&items[if timed { 3 } else { 2 }], scope, loops, false, &ty)?;
                    (
                        if head == "task.drop" || head == "task.cancel" {
                            Type::Unit
                        } else if timed {
                            Type::Value(tables.intern_value(ValueType::Option(payload.clone())))
                        } else {
                            payload
                        },
                        TaskJoin {
                            task: Box::new(task),
                            discard: head == "task.drop" || head == "task.cancel",
                            cancel: head == "task.cancel",
                            timeout,
                        },
                    )
                }
                "task.scope" => {
                    tables.count(node, items, 3)?;
                    let annotation = tables.list(&items[1])?;
                    tables.count(&items[1], annotation, 2)?;
                    if items[1].head() != Some("result") {
                        return Err(tables.error(&items[1], "task.scope requires (result T)"));
                    }
                    let result = tables.payload_ty(&annotation[1])?;
                    let body = self.expr_as(&items[2], scope, loops, tail, &result)?;
                    (
                        result,
                        TaskScope {
                            body: Box::new(body),
                        },
                    )
                }
                "resource.handle" | "resource.close" => {
                    tables.count(node, items, 2)?;
                    let value = self.expr(&items[1], scope, loops, false)?;
                    if head == "resource.handle" {
                        if !matches!(value.ty, Type::Borrow(_)) {
                            return Err(tables.error(node, "resource.handle requires a borrow"));
                        }
                        (Type::Num(NumType::I32), ResourceHandle(Box::new(value)))
                    } else {
                        if !matches!(value.ty, Type::Resource(_)) {
                            return Err(
                                tables.error(node, "resource.close consumes an owned resource")
                            );
                        }
                        (Type::Unit, ResourceClose(Box::new(value)))
                    }
                }
                "using" => {
                    tables.count(node, items, 4)?;
                    let value = self.expr(&items[2], scope, loops, false)?;
                    let Type::Resource(id) = value.ty else {
                        return Err(tables.error(node, "using requires a resource"));
                    };
                    let mut inner = scope.clone();
                    let name = tables.name(&items[1])?;
                    let local = self.local(
                        name.clone(),
                        items[1].span,
                        Type::Borrow(id),
                        BindingKind::Let,
                    );
                    inner.insert(name, local);
                    let body = self.expr_expected(&items[3], &inner, loops, tail, expected)?;
                    (
                        body.ty.clone(),
                        Using {
                            local,
                            value: Box::new(value),
                            body: Box::new(body),
                        },
                    )
                }
                "list.new" | "list.len" | "list.get" | "list.slice" | "list.from-array" => {
                    self.list_expr(node, items, scope, loops)?
                }
                "own.close" => {
                    count(2)?;
                    let value = self.expr(&items[1], scope, loops, false)?;
                    if !matches!(self.tables.value(&value.ty), Some(ValueType::Own(_))) {
                        return Err(tables.error(node, "own.close requires an own value"));
                    }
                    (Type::Unit, Close(Box::new(value)))
                }
                "newtype.wrap" | "newtype.unwrap" => {
                    count(3)?;
                    let ty = tables.type_id(&items[1])?;
                    let scalar = tables
                        .newtypes
                        .get(&ty)
                        .copied()
                        .ok_or_else(|| tables.error(&items[1], "expected a scalar newtype"))?;
                    let unwrap = head == "newtype.unwrap";
                    let input = if unwrap {
                        Type::Newtype(ty)
                    } else {
                        Type::Num(scalar)
                    };
                    let value = self.expr_as(&items[2], scope, loops, false, &input)?;
                    (
                        if unwrap {
                            Type::Num(scalar)
                        } else {
                            Type::Newtype(ty)
                        },
                        Newtype {
                            ty,
                            value: Box::new(value),
                            unwrap,
                        },
                    )
                }
                "let" => return self.let_expr(node, scope, loops, tail, expected),
                "block" => return self.block_expr(node, scope, loops, tail, expected),
                "return" => {
                    let result = self.function_result.clone();
                    let value = if result == Type::Unit && items.len() == 1 {
                        None
                    } else {
                        count(2)?;
                        Some(Box::new(
                            self.expr_as(&items[1], scope, loops, false, &result)?,
                        ))
                    };
                    (Type::Never, Return(value))
                }
                "br" | "br_if" => {
                    let name = items
                        .get(1)
                        .ok_or_else(|| tables.error(node, "missing branch label"))?;
                    let target = loops
                        .iter()
                        .rev()
                        .find(|l| name.atom() == Some(l.name.as_str()))
                        .cloned()
                        .ok_or_else(|| {
                            tables.error(name, "unknown branch label; use a named block or loop")
                        })?;
                    let parameters = if let Some(t) = &target.block {
                        if *t == Type::Unit {
                            vec![]
                        } else {
                            vec![t.clone()]
                        }
                    } else {
                        target
                            .bindings
                            .iter()
                            .map(|id| self.locals[id.0].ty.clone())
                            .collect::<Vec<_>>()
                    };
                    let conditional = head == "br_if";
                    if conditional && parameters.len() > 1 {
                        return Err(
                            tables.error(node, "typed br_if supports at most one branch value")
                        );
                    }
                    count(2 + parameters.len() + usize::from(conditional))?;
                    let values = self.expressions_as(
                        &items[2..2 + parameters.len()],
                        scope,
                        loops,
                        &parameters,
                    )?;
                    let condition = if conditional {
                        Some(Box::new(self.expr_as(
                            items.last().unwrap(),
                            scope,
                            loops,
                            false,
                            &Type::Num(NumType::I32),
                        )?))
                    } else {
                        None
                    };
                    let result = if conditional {
                        parameters.first().cloned().unwrap_or(Type::Unit)
                    } else {
                        Type::Never
                    };
                    (
                        result,
                        Branch {
                            label: target.id,
                            values,
                            condition,
                            bindings: target.bindings,
                            is_loop: target.block.is_none(),
                        },
                    )
                }
                "if" => return self.if_expr(node, scope, loops, tail, expected),
                "option.none" | "option.some" | "result.ok" | "result.err" | "pair.new"
                | "own.new" => self.value_expr(node, items, scope, loops, expected)?,
                "result.let" => self.result_let(node, items, scope, loops, tail, expected)?,
                "stream.fold-result" => {
                    return self.fold_result(node, items, scope, loops, tail, expected);
                }
                "match" => self.match_expr(node, items, scope, loops, tail, expected)?,
                "for" => self.for_expr(node, items, scope, loops)?,
                "for-each" => self.for_each(node, items, scope, loops)?,
                "try" => self.try_expr(node, items, scope, loops, tail)?,
                "throw" => {
                    let tag = tables.tag_id(
                        items
                            .get(1)
                            .ok_or_else(|| tables.error(node, "missing tag"))?,
                    )?;
                    let payloads = &self.module.tags[tag.0].payloads;
                    count(payloads.len() + 2)?;
                    let values = self.expressions_as(&items[2..], scope, loops, payloads)?;
                    (Type::Never, Throw { tag, values })
                }
                "variant.new" => {
                    if items.len() < 3 {
                        return Err(
                            tables.error(node, "expected (variant.new $Type case values...)")
                        );
                    }
                    let ty = tables.type_id(&items[1])?;
                    let (variant, definition) = self.variant(&items[2], ty)?;
                    count(definition.payloads.len() + 3)?;
                    let values =
                        self.expressions_as(&items[3..], scope, loops, &definition.payloads)?;
                    (
                        Type::Ref {
                            ty,
                            nullable: false,
                        },
                        VariantNew {
                            ty,
                            variant,
                            values,
                        },
                    )
                }
                "loop" | "native.loop" => self.loop_expr(node, items, scope, loops)?,
                "recur" => {
                    let name_node = items
                        .get(1)
                        .ok_or_else(|| self.tables.error(node, "missing loop name"))?;
                    let name = self.tables.atom(name_node)?;
                    let target = loops
                        .iter()
                        .rev()
                        .find(|l| l.name == name)
                        .ok_or_else(|| self.tables.error(node, "unknown loop"))?;
                    if !tail {
                        return Err(self.tables.error(node, "recur must be in tail position"));
                    }
                    if Some(target.id)
                        != loops.iter().rev().find(|l| l.block.is_none()).map(|l| l.id)
                        || target.block.is_some()
                    {
                        return Err(self
                            .tables
                            .error(node, "recur must target the innermost loop"));
                    }
                    count(target.bindings.len() + 2)?;
                    let parameters = target
                        .bindings
                        .iter()
                        .map(|id| self.locals[id.0].ty.clone())
                        .collect::<Vec<_>>();
                    let values = self.expressions_as(&items[2..], scope, loops, &parameters)?;
                    (
                        Type::Never,
                        Recur {
                            label: target.id,
                            assignments: target.bindings.iter().copied().zip(values).collect(),
                        },
                    )
                }
                "call" | "closure.call" => {
                    let name_node = items
                        .get(1)
                        .ok_or_else(|| self.tables.error(node, "missing call target"))?;
                    let name = self.tables.atom(name_node)?;
                    let id = FunctionId(*self.tables.functions.get(name).ok_or_else(|| {
                        self.tables
                            .error(node, format!("unknown function `{name}`"))
                    })?);
                    let function = &self.module.functions[id.0];
                    let mut expected = Vec::new();
                    match function.kind {
                        FunctionKind::Closure { environment } if head == "closure.call" => expected
                            .push(Type::Ref {
                                ty: environment,
                                nullable: false,
                            }),
                        FunctionKind::Closure { .. } => {
                            return Err(self.tables.error(node, "use closure.call for a closure"));
                        }
                        _ if head == "closure.call" => {
                            return Err(self.tables.error(node, "expected closure declaration"));
                        }
                        _ => {}
                    }
                    expected.extend(
                        function
                            .params
                            .iter()
                            .map(|id| function.locals[id.0].ty.clone()),
                    );
                    let result = function.result.clone();
                    if items.len() - 2 != expected.len() {
                        let mut error = tables
                            .error(
                                node,
                                format!(
                                    "argument count for `{name}`: expected {}, got {}",
                                    expected.len(),
                                    items.len() - 2
                                ),
                            )
                            .with_code("E_CALL_ARITY");
                        if let Some(site) = tables.function_sites.get(name) {
                            error = error.related(
                                tables.source,
                                site.name,
                                format!("`{name}` declares {} parameter(s)", function.params.len()),
                            );
                        }
                        if head == "closure.call" {
                            error = error.note(format!(
                                "expected 1 closure environment followed by {} argument(s); captures are stored in the environment",
                                function.params.len()
                            ));
                        }
                        return Err(error);
                    }
                    let parameters = function
                        .params
                        .iter()
                        .map(|id| function.locals[id.0].clone())
                        .collect::<Vec<_>>();
                    let mut args = Vec::new();
                    for (index, (argument, ty)) in items[2..].iter().zip(&expected).enumerate() {
                        let argument =
                            self.expr_as(argument, scope, loops, false, ty)
                                .map_err(|error| {
                                    let parameter = index
                                        .checked_sub(usize::from(head == "closure.call"))
                                        .and_then(|i| parameters.get(i).map(|p| (i, p)));
                                    if let Some((i, parameter)) = parameter {
                                        let message = format!(
                                            "argument {} of `{name}`: parameter `{}` requires {}",
                                            i + 1,
                                            parameter.name,
                                            tables.type_name(&parameter.ty)
                                        );
                                        error.expected_at(tables.source, parameter.span, message)
                                    } else {
                                        error.expected_at(
                                            tables.source,
                                            name_node.span,
                                            format!(
                                                "closure environment for `{name}` requires {}",
                                                tables.type_name(ty)
                                            ),
                                        )
                                    }
                                })?;
                        args.push(argument);
                    }
                    (result, Call { function: id, args })
                }
                "struct.named" => self.named_struct(node, items, scope, loops)?,
                "struct.update" => self.struct_update(node, items, scope, loops)?,
                "struct.new" | "closure.new" => {
                    let type_node = items
                        .get(1)
                        .ok_or_else(|| self.tables.error(node, "missing constructor type"))?;
                    let ty = self.tables.type_id(type_node)?;
                    let fields = match &self.module.types[ty.0].kind {
                        TypeKind::Struct(fields) if head == "struct.new" => fields.clone(),
                        TypeKind::Closure { captures, .. } if head == "closure.new" => {
                            captures.clone()
                        }
                        _ => {
                            return Err(self
                                .tables
                                .error(node, "constructor does not match type declaration"));
                        }
                    };
                    if head == "closure.new" && items.len() != fields.len() + 2 {
                        return Err(self.tables.error(
                            node,
                            format!(
                                "capture count: expected {}, got {}",
                                fields.len(),
                                items.len() - 2
                            ),
                        ));
                    }
                    count(fields.len() + 2)?;
                    let values = self.expressions_as(
                        &items[2..],
                        scope,
                        loops,
                        &fields.iter().map(|f| f.ty.clone()).collect::<Vec<_>>(),
                    )?;
                    (
                        Type::Ref {
                            ty,
                            nullable: false,
                        },
                        StructNew { ty, values },
                    )
                }
                "struct.get" | "struct.set" => {
                    count(if head == "struct.get" { 4 } else { 5 })?;
                    let ty = self.tables.type_id(&items[1])?;
                    let (index, field) = self.field(node, ty, &items[2])?;
                    let object = self.expr(&items[3], scope, loops, false)?;
                    self.check(&items[3], &Type::Ref { ty, nullable: true }, &object)?;
                    if head == "struct.get" {
                        (
                            field.ty,
                            StructGet {
                                ty,
                                field: index,
                                object: Box::new(object),
                            },
                        )
                    } else {
                        if !field.mutable {
                            return Err(self.tables.error(node, "cannot write immutable field"));
                        }
                        let value = self.expr_as(&items[4], scope, loops, false, &field.ty)?;
                        (
                            Type::Unit,
                            StructSet {
                                ty,
                                field: index,
                                object: Box::new(object),
                                value: Box::new(value),
                            },
                        )
                    }
                }
                "text.lit" => {
                    count(2)?;
                    let text = items[1].text().ok_or_else(|| {
                        self.tables.error(&items[1], "expected UTF-8 text literal")
                    })?;
                    (
                        Type::Value(self.tables.intern_value(ValueType::Text)),
                        Data(text.as_bytes().to_vec()),
                    )
                }
                "bytes.from-array"
                | "bytes.copy-to-array"
                | "bytes.new"
                | "bytes.len"
                | "bytes.get"
                | "bytes.slice"
                | "text.byte-len"
                | "text.bytes"
                | "text.from-bytes" => self.buffer_expr(node, items, scope, loops)?,
                "array.new" | "array.new_fixed" | "array.get" | "array.get_s" | "array.get_u"
                | "array.set" | "array.copy" => self.array_expr(node, items, scope, loops)?,
                "array.len" => {
                    count(2)?;
                    let object = self.expr(&items[1], scope, loops, false)?;
                    if let Type::Ref { ty, .. } = object.ty {
                        self.array(node, ty)?;
                    } else if object.ty != Type::Never {
                        return Err(self
                            .tables
                            .error(node, "array.len needs an array reference"));
                    }
                    (Type::Num(NumType::I32), ArrayLen(Box::new(object)))
                }
                "ref.null" => {
                    count(2)?;
                    let ty = self.tables.type_id(&items[1])?;
                    (Type::Ref { ty, nullable: true }, Null(ty))
                }
                "ref.is_null" | "ref.as_non_null" => {
                    count(2)?;
                    let value = self.expr(&items[1], scope, loops, false)?;
                    let Type::Ref { ty, .. } = value.ty else {
                        return Err(self.tables.error(node, "expected reference"));
                    };
                    if head == "ref.is_null" {
                        (Type::Num(NumType::I32), IsNull(Box::new(value)))
                    } else {
                        (
                            Type::Ref {
                                ty,
                                nullable: false,
                            },
                            AsNonNull(Box::new(value)),
                        )
                    }
                }
                "ref.eq" => {
                    count(3)?;
                    let a = self.expr(&items[1], scope, loops, false)?;
                    let b = self.expr(&items[2], scope, loops, false)?;
                    if !matches!(a.ty, Type::Ref { .. } | Type::Never)
                        || !matches!(b.ty, Type::Ref { .. } | Type::Never)
                    {
                        return Err(self.tables.error(node, "ref.eq needs references"));
                    }
                    (Type::Num(NumType::I32), RefEq(Box::new(a), Box::new(b)))
                }
                "drop" => {
                    count(2)?;
                    let value = self.expr(&items[1], scope, loops, false)?;
                    if value.ty == Type::Unit {
                        return Err(self.tables.error(node, "drop needs a value"));
                    }
                    (Type::Unit, Drop(Box::new(value)))
                }
                "nop" => {
                    count(1)?;
                    (Type::Unit, Nop)
                }
                "unreachable" => {
                    count(1)?;
                    (Type::Never, Unreachable)
                }
                _ => {
                    let surface = items
                        .first()
                        .and_then(|n| self.tables.source.get(n.span.start..n.span.end))
                        .filter(|s| s.starts_with('@'))
                        .unwrap_or(head);
                    let error = self
                        .tables
                        .error(node, format!("unsupported expression `{surface}`"));
                    return Err(if surface == "@string.len" {
                        error
                            .with_code("E_BUFFER_OP")
                            .help("use (@string.byte-len value) for the UTF-8 byte length")
                    } else {
                        error
                    });
                }
            }
        };
        Ok(Expr {
            ty,
            span: node.span,
            kind,
        })
    }

    fn result(&self, items: &[Sexp], start: usize) -> Result<(Type, usize), Error> {
        if let Some(node) = items.get(start).filter(|n| n.head() == Some("result")) {
            let parts = self.tables.list(node)?;
            self.tables.count(node, parts, 2)?;
            Ok((self.tables.ty(&parts[1])?, start + 1))
        } else {
            Ok((Type::Unit, start))
        }
    }

    fn list_expr(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
    ) -> Result<(Type, ExprKind), Error> {
        let tables = self.tables;
        let payload = tables.payload_ty(
            items
                .get(1)
                .ok_or_else(|| tables.error(node, "missing list element type"))?,
        )?;
        let list = tables.intern_value(ValueType::List(payload.clone()));
        let ty = Type::Value(list);
        let i32ty = Type::Num(NumType::I32);
        let (op, start, params, result) = match node.head().unwrap() {
            "list.new" => (ListOp::New, 2, vec![payload; items.len() - 2], ty),
            "list.len" => (ListOp::Len, 2, vec![ty], i32ty),
            "list.get" => (ListOp::Get, 2, vec![ty, i32ty], payload),
            "list.slice" => (ListOp::Slice, 2, vec![ty.clone(), i32ty.clone(), i32ty], ty),
            _ => {
                tables.count(node, items, 4)?;
                let array = tables.type_id(&items[2])?;
                if self.array(node, array)?.ty != payload
                    || self.array(node, array)?.packed.is_some()
                {
                    return Err(tables.error(node, "list.from-array element type mismatch"));
                }
                (
                    ListOp::FromArray(array),
                    3,
                    vec![Type::Ref {
                        ty: array,
                        nullable: false,
                    }],
                    ty,
                )
            }
        };
        tables.count(node, items, start + params.len())?;
        let args = self.expressions_as(&items[start..], scope, loops, &params)?;
        Ok((result, ExprKind::List { op, list, args }))
    }

    fn buffer_expr(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
    ) -> Result<(Type, ExprKind), Error> {
        let head = node.head().unwrap();
        if head == "bytes.from-array" || head == "bytes.copy-to-array" {
            let copy = head == "bytes.copy-to-array";
            if copy {
                self.tables.count(node, items, 7)?;
            } else if !matches!(items.len(), 3 | 5) {
                return Err(self.tables.error(
                    node,
                    "bytes.from-array expects array, optionally start and length",
                ));
            }
            let ty = self.tables.type_id(&items[1])?;
            let field = self.array(node, ty)?;
            if field.ty != Type::Num(NumType::I32) {
                return Err(self
                    .tables
                    .error(node, "bytes.from-array needs an i32 array"));
            }
            if copy && (!field.mutable || field.packed != Some(Packed::I8)) {
                return Err(self
                    .tables
                    .error(node, "bytes.copy-to-array requires a mutable i8 array"));
            }
            let value = self.expr(&items[2], scope, loops, false)?;
            self.check(
                &items[2],
                &Type::Ref {
                    ty,
                    nullable: false,
                },
                &value,
            )?;
            let buffer = self.tables.intern_value(ValueType::Bytes);
            let mut args = vec![value];
            for (index, argument) in items.iter().enumerate().skip(3) {
                let expected = if copy && index == 4 {
                    Type::Value(buffer)
                } else {
                    Type::Num(NumType::I32)
                };
                args.push(self.expr_as(argument, scope, loops, false, &expected)?);
            }
            return Ok((
                if copy {
                    Type::Unit
                } else {
                    Type::Value(buffer)
                },
                ExprKind::Buffer {
                    op: if copy {
                        BufferOp::CopyToArray(ty)
                    } else {
                        BufferOp::FromArray(ty)
                    },
                    buffer,
                    args,
                },
            ));
        }
        let bytes = Type::Value(self.tables.intern_value(ValueType::Bytes));
        let text = Type::Value(self.tables.intern_value(ValueType::Text));
        let i32ty = Type::Num(NumType::I32);
        let (op, params, result) = match head {
            "bytes.new" => (BufferOp::New, vec![i32ty; items.len() - 1], bytes),
            "bytes.len" => (BufferOp::Len, vec![bytes], i32ty),
            "text.byte-len" => (BufferOp::Len, vec![text], i32ty),
            "bytes.get" => (BufferOp::Get, vec![bytes, i32ty.clone()], i32ty),
            "bytes.slice" => (
                BufferOp::Slice,
                vec![bytes.clone(), i32ty.clone(), i32ty],
                bytes,
            ),
            "text.bytes" => (BufferOp::Encode, vec![text], bytes),
            _ => (
                BufferOp::Decode,
                vec![bytes],
                Type::Value(self.tables.intern_value(ValueType::Option(text))),
            ),
        };
        self.tables.count(node, items, params.len() + 1)?;
        let args = self.expressions_as(&items[1..], scope, loops, &params)?;
        let buffer = if let Some(Type::Value(id)) = params.first() {
            *id
        } else {
            self.tables.intern_value(ValueType::Bytes)
        };
        Ok((result, ExprKind::Buffer { op, buffer, args }))
    }

    fn delivery_expr(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        op: DeliveryOp,
        scope: &Scope,
        loops: &[LoopContext],
    ) -> Result<(Type, ExprKind), Error> {
        let payload = self.tables.payload_ty(
            items
                .get(1)
                .ok_or_else(|| self.tables.error(node, "missing delivery payload type"))?,
        )?;
        let shape = if op.is_future() {
            ValueType::Future(payload.clone())
        } else {
            ValueType::Stream(payload.clone())
        };
        let endpoint = self.tables.intern_value(shape.clone());
        let handle = Type::Value(endpoint);
        let mut parameters = Vec::new();
        if op == DeliveryOp::StreamReadBatch {
            parameters.push(Type::Num(NumType::I32));
        }
        if matches!(op, DeliveryOp::FutureReady | DeliveryOp::StreamCons) && payload != Type::Unit {
            parameters.push(payload.clone());
        }
        if !matches!(op, DeliveryOp::FutureReady | DeliveryOp::StreamEmpty) {
            parameters.push(handle.clone());
        }
        self.tables.count(node, items, 2 + parameters.len())?;
        let args = self.expressions_as(&items[2..], scope, loops, &parameters)?;
        let result = match op {
            DeliveryOp::FutureReady | DeliveryOp::StreamEmpty | DeliveryOp::StreamCons => handle,
            DeliveryOp::FutureAwait => payload,
            DeliveryOp::FutureClose | DeliveryOp::StreamClose => Type::Unit,
            DeliveryOp::StreamReadBatch => {
                let list = self.tables.intern_value(ValueType::List(payload));
                Type::Value(self.tables.intern_value(ValueType::Pair {
                    first: handle,
                    second: Type::Value(list),
                }))
            }
            DeliveryOp::StreamNext => {
                let option = self.tables.intern_value(ValueType::Option(payload));
                Type::Value(self.tables.intern_value(ValueType::Pair {
                    first: handle,
                    second: Type::Value(option),
                }))
            }
        };
        Ok((result, ExprKind::Delivery { op, endpoint, args }))
    }

    fn value_expr(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
        expected: Option<&Type>,
    ) -> Result<(Type, ExprKind), Error> {
        let head = node.head().unwrap();
        let explicit = crate::type_syntax::explicit_constructor(
            items,
            |n| self.tables.is_payload_syntax(n),
            |n| n.atom().is_some_and(|name| scope.contains_key(name)),
        );
        let (shape, start, variant) = if !explicit {
            let shape = expected
                .and_then(|ty| self.tables.value(ty))
                .filter(|shape| {
                    matches!(
                        (head, shape),
                        ("own.new", ValueType::Own(_))
                            | ("pair.new", ValueType::Pair { .. })
                            | ("option.some" | "option.none", ValueType::Option(_))
                            | ("result.ok" | "result.err", ValueType::Result { .. })
                    )
                })
                .ok_or_else(|| {
                    self.tables.error(
                        node,
                        "omitted constructor types require an expected type; add a type annotation",
                    )
                })?;
            (
                shape,
                1,
                usize::from(matches!(head, "option.some" | "result.err")),
            )
        } else if head == "own.new" {
            let payload = items
                .get(1)
                .ok_or_else(|| self.tables.error(node, "missing own payload type"))?;
            (ValueType::Own(self.tables.payload_ty(payload)?), 2, 0)
        } else if head == "pair.new" {
            if items.len() < 3 {
                return Err(self.tables.error(node, "missing pair payload types"));
            }
            (
                ValueType::Pair {
                    first: self.tables.payload_ty(&items[1])?,
                    second: self.tables.payload_ty(&items[2])?,
                },
                3,
                0,
            )
        } else if head.starts_with("option.") {
            let payload = items
                .get(1)
                .ok_or_else(|| self.tables.error(node, "missing option payload type"))?;
            (
                ValueType::Option(self.tables.payload_ty(payload)?),
                2,
                usize::from(head == "option.some"),
            )
        } else {
            if items.len() < 3 {
                return Err(self.tables.error(node, "missing result payload types"));
            }
            (
                ValueType::Result {
                    ok: self.tables.payload_ty(&items[1])?,
                    err: self.tables.payload_ty(&items[2])?,
                },
                3,
                usize::from(head == "result.err"),
            )
        };
        let ty = self.tables.intern_value(shape.clone());
        let variants = shape.variants(ty);
        let payloads = &variants[variant].payloads;
        self.tables.count(node, items, start + payloads.len())?;
        let values = self.expressions_as(&items[start..], scope, loops, payloads)?;
        Ok((
            Type::Value(ty),
            ExprKind::VariantNew {
                ty,
                variant,
                values,
            },
        ))
    }

    fn match_expr(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
        expected: Option<&Type>,
    ) -> Result<(Type, ExprKind), Error> {
        let tables = self.tables;
        let name = items
            .get(1)
            .ok_or_else(|| tables.error(node, "missing match type"))?;
        let (ty, result, start, subject) =
            if matches!(items.get(2).and_then(Sexp::head), Some("case" | "default")) {
                let result = expected.cloned().ok_or_else(|| {
                    tables.error(
                        node,
                        "match output needs an expected type; add a type annotation",
                    )
                })?;
                let subject = self.expr(name, scope, loops, false)?;
                let ty = match subject.ty {
                    Type::Value(ty)
                    | Type::Ref {
                        ty,
                        nullable: false,
                    } => ty,
                    _ => {
                        return Err(
                            tables.error(name, "match subject requires a non-null variant value")
                        );
                    }
                };
                (ty, result, 1, subject)
            } else {
                let ty = if name.atom().is_some() {
                    tables.type_id(name)?
                } else {
                    let Type::Value(ty) = tables.ty(name)? else {
                        return Err(tables.error(name, "expected variant, option, or result type"));
                    };
                    ty
                };
                let (result, start) = self.result(items, 2)?;
                let subject_node = items
                    .get(start)
                    .ok_or_else(|| tables.error(node, "missing match subject"))?;
                let subject_type = if ty.0 >= self.tables.types.len() {
                    Type::Value(ty)
                } else {
                    self.module.instance_type(ty)
                };
                let subject = self.expr_as(subject_node, scope, loops, false, &subject_type)?;
                (ty, result, start, subject)
            };
        let variant_count = self.variants(name, ty)?.len();
        let mut seen = BTreeSet::new();
        let mut arms = Vec::new();
        let clauses = &items[start + 1..];
        for (index, arm) in clauses.iter().enumerate() {
            let parts = tables.list(arm)?;
            if arm.head() == Some("default") {
                let fail = |message| tables.error(arm, message).with_code("E_MATCH_DEFAULT");
                if parts.len() != 2 {
                    return Err(fail("expected (@default body) without payload bindings"));
                }
                if index + 1 != clauses.len() {
                    return Err(fail("@default must be unique and the final match clause"));
                }
                let missing = self
                    .variants(name, ty)?
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| !seen.contains(index))
                    .map(|(index, variant)| (index, variant.clone()))
                    .collect::<Vec<_>>();
                if missing.is_empty() {
                    return Err(fail("@default must cover at least one remaining variant"));
                }
                if missing
                    .iter()
                    .any(|(_, variant)| variant.payloads.iter().any(|ty| !self.shared_data(ty)))
                {
                    return Err(fail(
                        "@default cannot discard owned or borrowed payloads; handle these variants with @case",
                    ));
                }
                let body = self.expr_as(&parts[1], scope, loops, tail, &result)?;
                // Defaults elaborate once, then become ordinary checked arms.
                // Bound the multiplication, including nested default bodies.
                let mut remaining = 100_000 / missing.len();
                let mut pending = vec![&body];
                while let Some(expr) = pending.pop() {
                    if remaining == 0 {
                        return Err(fail(
                            "@default expansion limit exceeded; move the body into a function",
                        ));
                    }
                    remaining -= 1;
                    pending.extend(expr.children());
                }
                for (variant, _) in missing {
                    seen.insert(variant);
                    arms.push(MatchArm {
                        variant,
                        bindings: vec![],
                        body: body.clone(),
                    });
                }
                continue;
            }
            tables.count(arm, parts, 4)?;
            if arm.head() != Some("case") {
                return Err(tables.error(
                    arm,
                    "expected (@case variant ($bindings...) body) or (@default body)",
                ));
            }
            let (variant, definition) = self.variant(&parts[1], ty)?;
            if !seen.insert(variant) {
                return Err(tables.error(arm, "duplicate match case"));
            }
            let (nested, bindings) =
                self.payload_bindings(&parts[2], &definition.payloads, scope)?;
            let body = self.expr_as(&parts[3], &nested, loops, tail, &result)?;
            arms.push(MatchArm {
                variant,
                bindings,
                body,
            });
        }
        if seen.len() != variant_count {
            let missing = self
                .variants(name, ty)?
                .iter()
                .enumerate()
                .filter(|(index, _)| !seen.contains(index))
                .map(|(_, variant)| variant.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(tables.error(node, format!("non-exhaustive match: missing {missing}")));
        }
        Ok((
            result,
            ExprKind::Match {
                ty,
                subject: Box::new(subject),
                arms,
            },
        ))
    }

    fn for_expr(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
    ) -> Result<(Type, ExprKind), Error> {
        use crate::sexpr::Kind;
        // Build hygienic core control flow; only the user's declared loop name
        // and state are visible. The generated recur has no source spelling.
        let synthetic = Span {
            start: node.span.start,
            end: node.span.start,
        };
        let atom = |text: &str| Sexp {
            context: Default::default(),
            kind: Kind::Atom(text.into()),
            span: synthetic,
        };
        let list = |items: Vec<Sexp>| Sexp {
            context: Default::default(),
            kind: Kind::List(items),
            span: synthetic,
        };
        let mut start = 2;
        while items.get(start).is_some_and(|n| n.head() == Some("bind")) {
            start += 1;
        }
        if start == 2 || items.len() != start + 3 {
            return Err(self.tables.error(node, "expected (for $label (bind $state T initial)... (while condition) (step next...) unit-body)"));
        }
        let condition = self.tables.branch(&items[start], "while")?.clone();
        let step = self.tables.list(&items[start + 1])?;
        if items[start + 1].head() != Some("step") {
            return Err(self
                .tables
                .error(&items[start + 1], "expected (step next-values...)"));
        }
        let name = items
            .get(1)
            .ok_or_else(|| self.tables.error(node, "missing for label"))?
            .clone();
        let mut recur = vec![atom("recur"), name.clone()];
        recur.extend_from_slice(&step[1..]);
        let body = list(vec![atom("block"), items[start + 2].clone(), list(recur)]);
        let condition = list(vec![
            atom("if"),
            condition,
            list(vec![atom("then"), body]),
            list(vec![atom("else"), list(vec![atom("nop")])]),
        ]);
        let mut expanded = vec![atom("loop"), name];
        expanded.extend_from_slice(&items[2..start]);
        expanded.push(condition);
        self.loop_expr(node, &expanded, scope, loops)
    }

    fn try_expr(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
    ) -> Result<(Type, ExprKind), Error> {
        let tables = self.tables;
        let (result, start) = self.result(items, 1)?;
        let do_node = items
            .get(start)
            .ok_or_else(|| tables.error(node, "missing (do body)"))?;
        let body = self.expr_as(tables.branch(do_node, "do")?, scope, loops, tail, &result)?;
        let mut handlers = Vec::new();
        let mut seen = BTreeSet::new();
        let mut all = false;
        for handler in &items[start + 1..] {
            if all {
                return Err(tables.error(handler, "catch_all must be last"));
            }
            let parts = tables.list(handler)?;
            let mut nested = scope.clone();
            let mut bindings = Vec::new();
            let (tag, body_node) = match handler.head() {
                Some("catch") => {
                    tables.count(handler, parts, 4)?;
                    let tag = tables.tag_id(&parts[1])?;
                    if !seen.insert(tag) {
                        return Err(tables.error(handler, "duplicate catch"));
                    }
                    let payloads = self.module.tags[tag.0].payloads.clone();
                    let names = tables.list(&parts[2])?;
                    if names.len() != payloads.len() {
                        return Err(
                            tables.error(&parts[2], "payload binding count does not match tag")
                        );
                    }
                    let mut seen_names = BTreeSet::new();
                    for (name, ty) in names.iter().zip(payloads) {
                        let spelling = tables.name(name)?;
                        if !seen_names.insert(spelling.clone()) {
                            return Err(tables.error(name, "duplicate catch binding"));
                        }
                        let id = self.local(spelling.clone(), name.span, ty, BindingKind::Catch);
                        nested.insert(spelling, id);
                        bindings.push(id);
                    }
                    (Some(tag), &parts[3])
                }
                Some("catch_all") => {
                    tables.count(handler, parts, 2)?;
                    all = true;
                    (None, &parts[1])
                }
                _ => return Err(tables.error(handler, "expected catch or catch_all")),
            };
            let body = self.expr_as(body_node, &nested, loops, tail, &result)?;
            handlers.push(CatchArm {
                tag,
                bindings,
                body,
            });
        }
        Ok((
            result,
            ExprKind::Try {
                body: Box::new(body),
                handlers,
            },
        ))
    }

    fn loop_expr(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
    ) -> Result<(Type, ExprKind), Error> {
        let name_node = items
            .get(1)
            .ok_or_else(|| self.tables.error(node, "missing loop name"))?;
        let name = self.tables.name(name_node)?;
        let id = LoopId(self.next_loop);
        self.next_loop += 1;
        let mut start = 2;
        let mut bindings = Vec::new();
        let mut nested = scope.clone();
        let mut names = BTreeSet::new();
        while let Some(binding) = items.get(start).filter(|n| n.head() == Some("bind")) {
            let parts = self.tables.list(binding)?;
            self.tables.count(binding, parts, 4)?;
            let name = self.tables.name(&parts[1])?;
            if !names.insert(name.clone()) {
                return Err(self.tables.error(binding, "duplicate loop binding"));
            }
            let ty = self.tables.ty(&parts[2])?;
            let initial = self.expr_as(&parts[3], scope, loops, false, &ty)?;
            let local = self.local(name.clone(), parts[1].span, ty, BindingKind::Loop(id));
            nested.insert(name, local);
            bindings.push((local, initial));
            start += 1;
        }
        let (result, start) = self.result(items, start)?;
        self.tables.count(node, items, start + 1)?;
        let mut nested_loops = loops.to_vec();
        nested_loops.push(LoopContext {
            name,
            id,
            bindings: bindings.iter().map(|(id, _)| *id).collect(),
            block: None,
        });
        let body = self.expr_as(&items[start], &nested, &nested_loops, true, &result)?;
        Ok((
            result,
            ExprKind::Loop {
                label: id,
                bindings,
                body: Box::new(body),
            },
        ))
    }

    fn array_expr(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
    ) -> Result<(Type, ExprKind), Error> {
        use ExprKind::*;
        let head = node.head().unwrap();
        let ty = self.tables.type_id(
            items
                .get(1)
                .ok_or_else(|| self.tables.error(node, "missing array type"))?,
        )?;
        let field = self.array(node, ty)?;
        match head {
            "array.copy" => {
                self.tables.count(node, items, 8)?;
                let source = self.tables.type_id(&items[2])?;
                let from = self.array(node, source)?;
                if !field.mutable
                    || field.ty != from.ty
                    || field.packed != from.packed
                    || !matches!(field.ty, Type::Num(_))
                {
                    return Err(self.tables.error(node, "array.copy requires a mutable destination and matching numeric storage types"));
                }
                let params = [
                    Type::Ref { ty, nullable: true },
                    Type::Num(NumType::I32),
                    Type::Ref {
                        ty: source,
                        nullable: true,
                    },
                    Type::Num(NumType::I32),
                    Type::Num(NumType::I32),
                ];
                let args = self.expressions_as(&items[3..], scope, loops, &params)?;
                Ok((
                    Type::Unit,
                    ArrayCopy {
                        target: ty,
                        source,
                        args,
                    },
                ))
            }
            "array.new" => {
                self.tables.count(node, items, 4)?;
                let initial = self.expr_as(&items[2], scope, loops, false, &field.ty)?;
                let length =
                    self.expr_as(&items[3], scope, loops, false, &Type::Num(NumType::I32))?;
                Ok((
                    Type::Ref {
                        ty,
                        nullable: false,
                    },
                    ArrayNew {
                        ty,
                        initial: Box::new(initial),
                        length: Box::new(length),
                    },
                ))
            }
            "array.new_fixed" => {
                let count_node = items
                    .get(2)
                    .ok_or_else(|| self.tables.error(node, "missing array length"))?;
                let count = self
                    .tables
                    .atom(count_node)?
                    .parse::<usize>()
                    .map_err(|_| {
                        self.tables
                            .error(count_node, "expected nonnegative array length")
                    })?;
                if count != items.len().saturating_sub(3) {
                    return Err(self
                        .tables
                        .error(node, "array length does not match element count"));
                }
                let values = self.expressions_as(
                    &items[3..],
                    scope,
                    loops,
                    &vec![field.ty.clone(); items.len() - 3],
                )?;
                Ok((
                    Type::Ref {
                        ty,
                        nullable: false,
                    },
                    ArrayFixed { ty, values },
                ))
            }
            "array.get" | "array.get_s" | "array.get_u" | "array.set" => {
                let get = head != "array.set";
                self.tables.count(node, items, if get { 4 } else { 5 })?;
                if get && (field.packed.is_some() == (head == "array.get")) {
                    return Err(self.tables.error(node, "packed arrays require array.get_s or array.get_u; unpacked arrays require array.get"));
                }
                let object = self.expr(&items[2], scope, loops, false)?;
                let index =
                    self.expr_as(&items[3], scope, loops, false, &Type::Num(NumType::I32))?;
                self.check(&items[2], &Type::Ref { ty, nullable: true }, &object)?;
                if get {
                    Ok((
                        field.ty,
                        ArrayGet {
                            ty,
                            signed: if head == "array.get" {
                                None
                            } else {
                                Some(head == "array.get_s")
                            },
                            object: Box::new(object),
                            index: Box::new(index),
                        },
                    ))
                } else {
                    if !field.mutable {
                        return Err(self.tables.error(node, "cannot write immutable array"));
                    }
                    let value = self.expr_as(&items[4], scope, loops, false, &field.ty)?;
                    Ok((
                        Type::Unit,
                        ArraySet {
                            ty,
                            object: Box::new(object),
                            index: Box::new(index),
                            value: Box::new(value),
                        },
                    ))
                }
            }
            _ => unreachable!(),
        }
    }
}
