use crate::Error;
use crate::ir::*;
use crate::sexpr::Sexp;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

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
    types: Names,
    functions: Names,
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
            types: Names::new(),
            functions: Names::new(),
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
            return Err(self.error(
                node,
                "expected numeric type, reference, bytes, text, or constructed value type",
            ));
        }
        let items = self.list(node)?;
        match items {
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
            Some("option") => {
                self.count(node, items, 2)?;
                Some(ValueType::Option(self.payload_ty(&items[1])?))
            }
            Some("own") => {
                self.count(node, items, 2)?;
                Some(ValueType::Own(self.payload_ty(&items[1])?))
            }
            Some("future" | "stream") => {
                self.count(node, items, 2)?;
                let payload = self.payload_ty(&items[1])?;
                Some(if node.head() == Some("future") {
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
        for child in items {
            self.collect_values(child)?;
        }
        if let Some(value) = self.value_shape(node)? {
            self.intern_value(value);
        }
        match node.head() {
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
            s == "unit" || s == "bytes" || s == "text" || NumType::parse(s).is_some()
        }) || matches!(
            node.head(),
            Some("ref" | "option" | "own" | "future" | "stream" | "pair")
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
        Ok(Field { name, ty, mutable })
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

pub(crate) fn check(source: &str, root: &Sexp) -> Result<Module, Error> {
    let mut tables = Tables::new(source);
    let declarations = tables.list(root)?;
    if root.head() != Some("module") {
        return Err(tables.error(root, "expected (module ...)"));
    }
    let mut type_nodes = Vec::new();
    let mut function_nodes = Vec::new();
    let mut test_nodes = Vec::new();
    let mut law_nodes = Vec::new();
    let mut law_names = BTreeSet::new();
    let mut test_names = BTreeSet::new();
    let mut tag_nodes = Vec::new();
    for node in &declarations[1..] {
        let items = tables.list(node)?;
        match node.head() {
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
                if items[3].head() != Some("func") {
                    return Err(tables.error(node, "only function imports are supported"));
                }
                function_nodes.push((&items[3], Some((module.to_owned(), name.to_owned()))));
            }
            _ => return Err(tables.error(node, "unsupported declaration")),
        }
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
    tables.collect_values(root)?;
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
                    TypeKind::Array(tables.field(None, &definition[1])?)
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
        types.push(TypeDecl { name, kind });
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
                Some("effects") if body.is_none() => {
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
                        return Err(
                            tables.error(item, "expected one folded expression as function body")
                        );
                    }
                    if item.list().is_none() {
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
        let base = format!("__gcwat_test_{index}");
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
            effect_contract: None,
        });
        bodies.push(Some(&items[2]));
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
            effect_contract: Some(crate::effects::EffectContract {
                span: node.span,
                allowed: crate::effects::EffectSet::default(),
            }),
        });
        bodies.push(items.last());
    }
    let mut module = Module {
        types,
        functions,
        tests,
        laws,
        tags,
    };
    // Body checking sees signatures only: no dependency on declaration order.
    let headers = module.clone();
    for (index, body) in bodies.into_iter().enumerate() {
        if matches!(module.functions[index].kind, FunctionKind::Import { .. }) {
            continue;
        }
        let function = &headers.functions[index];
        let mut checker = Checker {
            tables: &tables,
            module: &headers,
            locals: function.locals.clone(),
            next_loop: 0,
            remaining_work: 1_000_000,
            closure: matches!(function.kind, FunctionKind::Closure { .. }),
        };
        let scope = checker
            .locals
            .iter()
            .enumerate()
            .map(|(id, b)| (b.name.clone(), LocalId(id)))
            .collect();
        let expression = if let Some(body) = body {
            checker
                .expr_as(body, &scope, &[], true, &function.result)
                .map_err(|error| {
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
    }
    // Inference may compose value types that never appeared in annotations.
    module.types.truncate(tables.types.len());
    module
        .types
        .extend(tables.values.borrow().types.iter().cloned());
    Ok(module)
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
}

struct Checker<'a, 's> {
    tables: &'a Tables<'s>,
    module: &'a Module,
    locals: Vec<Binding>,
    next_loop: usize,
    remaining_work: usize,
    closure: bool,
}

impl Checker<'_, '_> {
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
            Some(ValueType::Own(_) | ValueType::Future(_) | ValueType::Stream(_)) => true,
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
        let result = self
            .expr_expected(node, scope, loops, false, expected)
            .map(|e| e.ty);
        self.locals.truncate(locals);
        self.next_loop = next_loop;
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
                ValueType::Bytes
                | ValueType::Text
                | ValueType::Future(_)
                | ValueType::Stream(_) => Err(self
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

    fn expr_expected(
        &mut self,
        node: &Sexp,
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
        expected: Option<&Type>,
    ) -> Result<Expr, Error> {
        use ExprKind::*;
        self.remaining_work = self.remaining_work.checked_sub(1).ok_or_else(|| {
            self.tables
                .error(node, "local type elaboration limit exceeded")
        })?;
        let items = self.tables.list(node)?;
        let head = items
            .first()
            .and_then(Sexp::atom)
            .ok_or_else(|| self.tables.error(node, "expected expression operator"))?;
        let tables = self.tables;
        let count = |n| tables.count(node, items, n);
        let (ty, kind) = if let Some(op) = DeliveryOp::parse(head) {
            self.delivery_expr(node, items, op, scope, loops)?
        } else if let Some(op) = NumericOp::parse(head) {
            let signature = op.signature();
            count(signature.arity + 1)?;
            let args = self.expressions(&items[1..], scope, loops)?;
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
            let literal = self.tables.atom(&items[1])?.to_owned();
            let fragment = format!(
                "(module (func (result {}) ({} {})))",
                ty.wat(),
                head,
                literal
            );
            wat::parse_str(&fragment)
                .map_err(|_| self.tables.error(&items[1], "invalid numeric literal"))?;
            (Type::Num(ty), Const { ty, literal })
        } else {
            match head {
                "local.get" | "move" => {
                    count(2)?;
                    let name = self.tables.atom(&items[1])?;
                    let id = *scope
                        .get(name)
                        .ok_or_else(|| self.unknown_local(&items[1], name, scope))?;
                    (
                        self.locals[id.0].ty.clone(),
                        if head == "move" { Move(id) } else { Local(id) },
                    )
                }
                "own.close" => {
                    count(2)?;
                    let value = self.expr(&items[1], scope, loops, false)?;
                    if !matches!(self.tables.value(&value.ty), Some(ValueType::Own(_))) {
                        return Err(tables.error(node, "own.close requires an own value"));
                    }
                    (Type::Unit, Close(Box::new(value)))
                }
                "let" => {
                    if !(4..=5).contains(&items.len()) {
                        return Err(
                            tables.error(node, "expected (let $name [type] initializer body)")
                        );
                    }
                    if items[1].list().is_some() {
                        return self.pair_let(node, items, scope, loops, tail, expected);
                    }
                    let name = self.tables.name(&items[1])?;
                    let (annotation, start) = if items.len() == 5 {
                        (Some(tables.ty(&items[2])?), 3)
                    } else {
                        (None, 2)
                    };
                    let value = self.expr_expected(
                        &items[start],
                        scope,
                        loops,
                        false,
                        annotation.as_ref(),
                    )?;
                    if matches!(value.ty, Type::Unit | Type::Never) {
                        return Err(self
                            .tables
                            .error(&items[start], "let initializer must produce a value"));
                    }
                    let binding_type = annotation.unwrap_or_else(|| value.ty.clone());
                    self.check(&items[start], &binding_type, &value)?;
                    let id =
                        self.local(name.clone(), items[1].span, binding_type, BindingKind::Let);
                    let mut nested = scope.clone();
                    nested.insert(name, id);
                    let body =
                        self.expr_expected(&items[start + 1], &nested, loops, tail, expected)?;
                    (
                        body.ty.clone(),
                        Let {
                            local: id,
                            value: Box::new(value),
                            body: Box::new(body),
                        },
                    )
                }
                "block" => {
                    let (result, start) = self.result(items, 1)?;
                    let mut expressions = Vec::new();
                    for (i, expression) in items[start..].iter().enumerate() {
                        let last = i + start + 1 == items.len();
                        let expression = self.expr_expected(
                            expression,
                            scope,
                            loops,
                            last && tail,
                            Some(if last { &result } else { &Type::Unit }),
                        )?;
                        if !last && self.is_owned(&expression.ty) {
                            return Err(
                                tables.error(&items[start + i], "cannot discard an owned value")
                            );
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
                        result
                    };
                    (ty, Block(expressions))
                }
                "if" => {
                    let (result, start) = self.result(items, 1)?;
                    count(start + 3)?;
                    let condition = self.expr(&items[start], scope, loops, false)?;
                    self.check(&items[start], &Type::Num(NumType::I32), &condition)?;
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
                }
                "option.none" | "option.some" | "result.ok" | "result.err" | "pair.new"
                | "own.new" => self.value_expr(node, items, scope, loops, expected)?,
                "result.let" => self.result_let(node, items, scope, loops, tail, expected)?,
                "stream.fold-result" => {
                    return self.fold_result(node, items, scope, loops, tail, expected);
                }
                "match" => self.match_expr(node, items, scope, loops, tail, expected)?,
                "for" => self.for_expr(node, items, scope, loops)?,
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
                "loop" => self.loop_expr(node, items, scope, loops)?,
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
                    if Some(target.id) != loops.last().map(|l| l.id) {
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
                "bytes.from-array" | "bytes.new" | "bytes.len" | "bytes.get" | "bytes.slice"
                | "text.byte-len" | "text.bytes" | "text.from-bytes" => {
                    self.buffer_expr(node, items, scope, loops)?
                }
                "array.new" | "array.new_fixed" | "array.get" | "array.set" => {
                    self.array_expr(node, items, scope, loops)?
                }
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
                    return Err(self
                        .tables
                        .error(node, format!("unsupported expression `{head}`")));
                }
            }
        };
        Ok(Expr {
            ty,
            span: node.span,
            kind,
        })
    }

    fn expressions(
        &mut self,
        nodes: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
    ) -> Result<Vec<Expr>, Error> {
        nodes
            .iter()
            .map(|node| self.expr(node, scope, loops, false))
            .collect()
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

    fn buffer_expr(
        &mut self,
        node: &Sexp,
        items: &[Sexp],
        scope: &Scope,
        loops: &[LoopContext],
    ) -> Result<(Type, ExprKind), Error> {
        let head = node.head().unwrap();
        if head == "bytes.from-array" {
            self.tables.count(node, items, 3)?;
            let ty = self.tables.type_id(&items[1])?;
            if self.array(node, ty)?.ty != Type::Num(NumType::I32) {
                return Err(self
                    .tables
                    .error(node, "bytes.from-array needs an i32 array"));
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
            return Ok((
                Type::Value(buffer),
                ExprKind::Buffer {
                    op: BufferOp::FromArray(ty),
                    buffer,
                    args: vec![value],
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
        let explicit = match head {
            "pair.new" | "result.ok" | "result.err" => {
                items.len() >= 3
                    && self.tables.is_payload_syntax(&items[1])
                    && self.tables.is_payload_syntax(&items[2])
            }
            _ => items
                .get(1)
                .is_some_and(|n| self.tables.is_payload_syntax(n)),
        };
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
            if name.list().is_some() && !tables.is_payload_syntax(name) {
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
        for arm in &items[start + 1..] {
            let parts = tables.list(arm)?;
            tables.count(arm, parts, 4)?;
            if arm.head() != Some("case") {
                return Err(tables.error(arm, "expected (case variant ($bindings...) body)"));
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
            kind: Kind::Atom(text.into()),
            span: synthetic,
        };
        let list = |items: Vec<Sexp>| Sexp {
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
            "array.new" => {
                self.tables.count(node, items, 4)?;
                let initial = self.expr_as(&items[2], scope, loops, false, &field.ty)?;
                let length = self.expr(&items[3], scope, loops, false)?;
                self.check(&items[3], &Type::Num(NumType::I32), &length)?;
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
            "array.get" | "array.set" => {
                self.tables
                    .count(node, items, if head == "array.get" { 4 } else { 5 })?;
                let object = self.expr(&items[2], scope, loops, false)?;
                let index = self.expr(&items[3], scope, loops, false)?;
                self.check(&items[2], &Type::Ref { ty, nullable: true }, &object)?;
                self.check(&items[3], &Type::Num(NumType::I32), &index)?;
                if head == "array.get" {
                    Ok((
                        field.ty,
                        ArrayGet {
                            ty,
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
