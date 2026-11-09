//! Static namespaces. Collect definitions, resolve imports against those
//! definitions, then rewrite declarations before the ordinary type checker.
use crate::Error;
use crate::sexpr::{Kind, Sexp};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum SymbolKind {
    Macro,
    Function,
    Memory,
    Cell,
    Tag,
    Type,
    Region,
}
impl SymbolKind {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "macro" => Some(Self::Macro),
            "func" => Some(Self::Function),
            "memory" => Some(Self::Memory),
            "cell" => Some(Self::Cell),
            "region" => Some(Self::Region),
            "tag" => Some(Self::Tag),
            "type" => Some(Self::Type),
            _ => None,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Macro => "macro",
            Self::Function => "func",
            Self::Memory => "memory",
            Self::Cell => "cell",
            Self::Region => "region",
            Self::Tag => "tag",
            Self::Type => "type",
        }
    }
}
#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct Symbol {
    kind: SymbolKind,
    name: String,
}
impl Symbol {
    fn new(kind: SymbolKind, name: String) -> Self {
        Self { kind, name }
    }
}
#[derive(Default)]
struct Scope {
    declarations: Vec<Sexp>,
    macro_definitions: BTreeMap<String, Sexp>,
    definitions: BTreeMap<Symbol, String>,
    aliases: BTreeMap<Symbol, String>,
    public: BTreeMap<Symbol, crate::ir::Span>,
    opaque: BTreeSet<Symbol>,
    uses: Vec<Sexp>,
    file: Option<String>,
}
impl Scope {
    fn lookup(&self, symbol: &Symbol) -> Option<&String> {
        self.definitions
            .get(symbol)
            .or_else(|| self.aliases.get(symbol))
    }
}
type Scopes = BTreeMap<String, Scope>;

fn atom(node: &Sexp) -> String {
    node.atom().unwrap_or("").to_owned()
}
fn qualified(scope: &str, name: &str) -> String {
    if scope.is_empty() {
        name.into()
    } else {
        format!("${scope}.{}", name.trim_start_matches('$'))
    }
}
fn identifier(source: &str, node: &Sexp) -> Result<String, Error> {
    let n = atom(node);
    if !n.starts_with('$')
        || n.len() < 2
        || n.starts_with("$__")
        || !n[1..]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    {
        return Err(Error::at(source, node.span, "expected ASCII $identifier"));
    }
    Ok(n)
}
fn collect(
    source: &str,
    scope: &str,
    nodes: &[Sexp],
    scopes: &mut Scopes,
    file: Option<&str>,
) -> Result<(), Error> {
    let mut current = Scope {
        file: file.map(str::to_owned),
        ..Scope::default()
    };
    for node in nodes {
        let items = node.list().unwrap_or(&[]);
        match node.head() {
            Some("namespace" | "file.namespace") => {
                let name = items
                    .get(1)
                    .ok_or_else(|| Error::at(source, node.span, "missing namespace name"))?;
                let generated = node.head() == Some("file.namespace");
                let name = identifier(source, name)?;
                if generated && !scope.is_empty() {
                    return Err(Error::at(
                        source,
                        node.span,
                        "file modules belong to the compilation root",
                    ));
                }
                let path = qualified(scope, &name).trim_start_matches('$').to_owned();
                if scopes.contains_key(&path) || path == scope {
                    return Err(Error::at(source, node.span, "duplicate namespace"));
                }
                collect(
                    source,
                    &path,
                    &items[2..],
                    scopes,
                    if generated { Some(&path) } else { file },
                )?;
            }
            Some("public") | Some("export")
                if node.head() == Some("public") || items.get(1).and_then(Sexp::list).is_some() =>
            {
                for export in &items[1..] {
                    let parts = export.list().unwrap_or(&[]);
                    let kind = if export.head() == Some("opaque") {
                        Some(SymbolKind::Type)
                    } else {
                        export.head().and_then(SymbolKind::parse)
                    };
                    if parts.len() != 2 || kind.is_none() {
                        return Err(Error::at(
                            source,
                            export.span,
                            "expected (public (func/type/tag/memory $name)...)",
                        ));
                    }
                    let symbol = Symbol::new(kind.unwrap(), identifier(source, &parts[1])?);
                    if export.head() == Some("opaque") {
                        current
                            .opaque
                            .insert(Symbol::new(symbol.kind, symbol.name.clone()));
                    }
                    if current.public.insert(symbol, export.span).is_some() {
                        return Err(Error::at(
                            source,
                            export.span,
                            "duplicate public declaration",
                        ));
                    }
                }
            }
            Some("race-free") if !scope.is_empty() => {
                return Err(
                    Error::at(source, node.span, "%race-free belongs to the entry module")
                        .with_code("E_RACE_MODE"),
                );
            }
            Some("test") if file.is_some() => {
                return Err(
                    Error::at(source, node.span, "file modules cannot declare tests")
                        .help("put tests in a separate entry file that imports the library"),
                );
            }
            Some("bench") if file.is_some() => {
                return Err(
                    Error::at(source, node.span, "file modules cannot declare benchmarks")
                        .help("put benchmarks in a separate entry file that imports the library"),
                );
            }
            Some("use") if items.get(1).and_then(Sexp::text).is_some() => {
                return Err(
                    Error::at(source, node.span, "file uses require source::load")
                        .with_code("E_SOURCE_REQUIRED")
                        .help("load the entry file to resolve its relative %use paths"),
                );
            }
            Some("use") => current.uses.push(node.clone()),
            Some("import") if items.get(1).and_then(Sexp::atom).is_some() => {
                current.uses.push(node.clone())
            }
            Some("import") if items.get(2).and_then(Sexp::text).is_none() => {
                return Err(
                    Error::at(source, node.span, "file imports require source::load")
                        .help("load the entry file before checking or compiling its source"),
                );
            }
            _ => {
                let declaration = if node.head() == Some("import") {
                    items.get(3).unwrap_or(node)
                } else {
                    node
                };
                let kinds: &[SymbolKind] = match declaration.head() {
                    Some("macro") => &[SymbolKind::Macro],
                    Some("closure") => &[SymbolKind::Function, SymbolKind::Type],
                    Some("func") => &[SymbolKind::Function],
                    Some("memory") => &[SymbolKind::Memory],
                    Some("atomic.cell") => &[SymbolKind::Cell],
                    Some("readonly.region") => &[SymbolKind::Region],
                    Some("type" | "type.alias") => &[SymbolKind::Type],
                    Some("tag") => &[SymbolKind::Tag],
                    _ => &[],
                };
                if !kinds.is_empty() {
                    let name =
                        declaration.list().unwrap().get(1).ok_or_else(|| {
                            Error::at(source, node.span, "missing declaration name")
                        })?;
                    let name = identifier(source, name)?;
                    if declaration.head() == Some("macro") {
                        current
                            .macro_definitions
                            .insert(qualified(scope, &name), declaration.clone());
                    }
                    for kind in kinds {
                        if current
                            .definitions
                            .insert(Symbol::new(*kind, name.clone()), qualified(scope, &name))
                            .is_some()
                        {
                            return Err(Error::at(source, node.span, "duplicate declaration"));
                        }
                    }
                }
                current.declarations.push(node.clone());
            }
        }
    }
    for symbol in &current.opaque {
        if !current.definitions.contains_key(symbol) {
            return Err(Error::at(
                source,
                nodes.first().map(|n| n.span).unwrap_or_default(),
                "opaque exports require a local type definition",
            ));
        }
    }
    for declaration in &current.declarations {
        if declaration.head() == Some("type.alias")
            && let Some(name) = declaration.list().and_then(|p| p.get(1))
            && current
                .opaque
                .contains(&Symbol::new(SymbolKind::Type, atom(name)))
        {
            return Err(Error::at(
                source,
                declaration.span,
                "a transparent type alias cannot be opaque",
            ));
        }
    }
    if scopes.insert(scope.into(), current).is_some() {
        return Err(Error::message("duplicate namespace".into()));
    }
    Ok(())
}

fn import_aliases(
    source: &str,
    import: &Sexp,
    scopes: &Scopes,
    scope: &str,
) -> Result<Vec<(Symbol, String)>, Error> {
    let parts = import.list().unwrap();
    if parts.len() < 3 {
        return Err(Error::at(
            source,
            import.span,
            "expected (use $namespace (func/type/tag/memory $member [$alias])...)",
        ));
    }
    let namespace = identifier(source, &parts[1])?;
    let target_name = paths(scopes, scope, namespace.trim_start_matches('$'))
        .into_iter()
        .find(|p| scopes.contains_key(p))
        .ok_or_else(|| Error::at(source, import.span, "unknown namespace"))?;
    let target = &scopes[&target_name];
    let mut aliases = Vec::new();
    for binding in &parts[2..] {
        let parts = binding.list().unwrap_or(&[]);
        let kind = binding.head().and_then(SymbolKind::parse);
        if !(2..=3).contains(&parts.len()) || kind.is_none() {
            return Err(Error::at(
                source,
                binding.span,
                "expected (func/type/tag/memory $member [$alias])",
            ));
        }
        let symbol = Symbol::new(kind.unwrap(), identifier(source, &parts[1])?);
        if symbol.name.contains('.')
            && target.file.as_deref() == Some(target_name.as_str())
            && target.lookup(&symbol).is_none()
        {
            // A namespaced file export still requires an explicit named import.
            let definition = resolve(source, scopes, &target_name, symbol.kind, &parts[1])?;
            let alias = Symbol::new(
                symbol.kind,
                identifier(source, parts.get(2).unwrap_or(&parts[1]))?,
            );
            aliases.push((alias, definition));
            continue;
        }
        let definition = target
            .lookup(&symbol)
            .ok_or_else(|| Error::at(source, binding.span, "unknown imported member"))?;
        if !target.public.contains_key(&symbol) {
            return Err(Error::at(source, binding.span, "private module member"));
        }
        let alias = Symbol::new(
            symbol.kind,
            identifier(source, parts.get(2).unwrap_or(&parts[1]))?,
        );
        aliases.push((alias, definition.clone()));
    }
    Ok(aliases)
}
fn resolve_imports(source: &str, scopes: &mut Scopes) -> Result<(), Error> {
    // Diagnose impossible exports at the declaring file before an importer
    // reports them missing. Forwarded names may await their named imports.
    for scope in scopes.values() {
        for (symbol, span) in &scope.public {
            let planned = scope
                .uses
                .iter()
                .flat_map(|n| n.list().unwrap_or(&[]).iter().skip(2))
                .any(|n| {
                    let parts = n.list().unwrap_or(&[]);
                    n.head().and_then(SymbolKind::parse) == Some(symbol.kind)
                        && parts.get(2).or_else(|| parts.get(1)).and_then(Sexp::atom)
                            == Some(symbol.name.as_str())
                });
            if scope.lookup(symbol).is_none() && !planned {
                return Err(Error::at(
                    source,
                    *span,
                    format!("unknown public {} `{}`", symbol.kind.name(), symbol.name),
                ));
            }
        }
    }
    let mut pending = scopes
        .iter_mut()
        .map(|(name, scope)| (name.clone(), std::mem::take(&mut scope.uses)))
        .flat_map(|(name, imports)| imports.into_iter().map(move |i| (name.clone(), i)))
        .collect::<Vec<_>>();
    while !pending.is_empty() {
        let mut remaining = Vec::new();
        let mut first_error = None;
        let mut progress = false;
        for (name, import) in pending {
            let aliases = match import_aliases(source, &import, scopes, &name) {
                Ok(aliases) => aliases,
                Err(error) => {
                    first_error.get_or_insert(error);
                    remaining.push((name, import));
                    continue;
                }
            };
            let scope = scopes.get_mut(&name).unwrap();
            for (symbol, definition) in aliases {
                if scope.lookup(&symbol).is_some() {
                    return Err(Error::at(
                        source,
                        import.span,
                        "duplicate import or declaration",
                    ));
                }
                scope.aliases.insert(symbol, definition);
            }
            progress = true;
        }
        if !progress {
            return Err(first_error.unwrap());
        }
        pending = remaining;
    }
    for scope in scopes.values() {
        for (symbol, span) in &scope.public {
            if scope.lookup(symbol).is_none() {
                return Err(Error::at(
                    source,
                    *span,
                    format!("unknown public {} `{}`", symbol.kind.name(), symbol.name),
                ));
            }
        }
    }
    Ok(())
}
fn paths(scopes: &Scopes, scope: &str, path: &str) -> Vec<String> {
    if let Some(file) = &scopes[scope].file {
        let mut out = Vec::new();
        let mut ancestor = scope;
        loop {
            out.push(format!("{ancestor}.{path}"));
            if ancestor == file {
                break;
            }
            ancestor = ancestor.rsplit_once('.').unwrap().0;
        }
        // The loader's use declarations refer to other file identities directly.
        if scopes
            .get(path)
            .is_some_and(|s| s.file.as_deref() == Some(path))
        {
            out.push(path.into());
        }
        out
    } else {
        vec![path.into()]
    }
}
fn resolve(
    source: &str,
    scopes: &Scopes,
    scope: &str,
    kind: SymbolKind,
    node: &Sexp,
) -> Result<String, Error> {
    let symbol = Symbol::new(kind, atom(node));
    if let Some(target) = scopes[scope].lookup(&symbol) {
        return Ok(target.clone());
    }
    if let Some((path, member)) = symbol.name.trim_start_matches('$').rsplit_once('.') {
        for path in paths(scopes, scope, path) {
            let Some(target) = scopes.get(&path) else {
                continue;
            };
            if target.file != scopes[scope].file {
                return Err(Error::at(
                    source,
                    node.span,
                    "file module members require a named import",
                ));
            }
            let member = Symbol::new(kind, format!("${member}"));
            if let Some(name) = target.lookup(&member) {
                if scope == path || target.public.contains_key(&member) {
                    return Ok(name.clone());
                }
                return Err(Error::at(source, node.span, "private module member"));
            }
        }
    }
    Err(Error::at(
        source,
        node.span,
        format!("unknown {} `{}`", kind.name(), symbol.name),
    ))
}
fn rewrite(source: &str, scopes: &Scopes, scope: &str, node: &mut Sexp) -> Result<(), Error> {
    let head = node.head().unwrap_or("").to_owned();
    if head.starts_with('@') {
        return Ok(());
    }
    if head == "export" && !scope.is_empty() {
        return Err(Error::at(
            source,
            node.span,
            "Wasm exports belong to the root module",
        ));
    }
    let Kind::List(items) = &mut node.kind else {
        return Ok(());
    };
    if let Some(name) = head.strip_prefix('%').filter(|n| n.starts_with('$')) {
        let mut operand = items[0].clone();
        operand.kind = Kind::Atom(name.into());
        items[0].kind = Kind::Atom(format!(
            "%{}",
            resolve(source, scopes, scope, SymbolKind::Macro, &operand)
                .map_err(|e| e.with_code("E_MACRO_UNKNOWN"))?
        ));
        let concrete = items[0].atom().unwrap().trim_start_matches('%');
        let definition = scopes
            .values()
            .find_map(|s| s.macro_definitions.get(concrete));
        if let Some(definition) = definition {
            let mut call = Sexp {
                kind: Kind::List(items.clone()),
                span: node.span,
                context: node.context,
            };
            let spans = crate::macros::declaration(source, definition)?.type_arguments(&call);
            fn types(
                source: &str,
                scopes: &Scopes,
                scope: &str,
                n: &mut Sexp,
                spans: &[crate::ir::Span],
            ) -> Result<(), Error> {
                if spans.contains(&n.span) {
                    crate::type_syntax::map_type(n, &mut |ty| {
                        resolve_application(
                            source,
                            scopes,
                            scope,
                            SymbolKind::Type,
                            ty,
                            &BTreeSet::new(),
                        )
                    })?;
                } else if let Kind::List(parts) = &mut n.kind {
                    let start = usize::from(parts.first().and_then(Sexp::atom).is_some());
                    for child in parts.iter_mut().skip(start) {
                        types(source, scopes, scope, child, spans)?;
                    }
                }
                Ok(())
            }
            types(source, scopes, scope, &mut call, &spans)?;
            if let Kind::List(parts) = call.kind {
                *items = parts;
            }
        }
    }
    let kind = match head.as_str() {
        "macro" => Some(SymbolKind::Macro),
        "func" => Some(SymbolKind::Function),
        "memory" => Some(SymbolKind::Memory),
        "atomic.cell" | "atomic.at" => Some(SymbolKind::Cell),
        "readonly.region" | "readonly.load" => Some(SymbolKind::Region),
        "type" | "type.alias" | "closure" | "abi.type" => Some(SymbolKind::Type),
        "tag" | "throw" | "catch" => Some(SymbolKind::Tag),
        _ => None,
    };
    if let Some(kind) = kind
        && let Some(name) = items.get_mut(1)
    {
        resolve_application(source, scopes, scope, kind, name, &BTreeSet::new())?;
    }
    if crate::wasm::supported(&head) {
        let required = if head == "memory.copy" { 2 } else { 1 };
        let mut found = 0;
        for operand in items.iter_mut().skip(1) {
            if found == required {
                break;
            }
            let Some(name) = operand.atom().filter(|n| n.starts_with('$')) else {
                break;
            };
            if scopes[scope]
                .lookup(&Symbol::new(SymbolKind::Memory, name.to_owned()))
                .is_none()
            {
                break;
            }
            resolve_application(
                source,
                scopes,
                scope,
                SymbolKind::Memory,
                operand,
                &BTreeSet::new(),
            )?;
            found += 1;
        }
        if !scope.is_empty() && found != required {
            return Err(Error::at(
                source,
                node.span,
                "file modules require named memory operands",
            )
            .with_code("E_MEMORY_USE"));
        }
    }
    if crate::threads::lookup(&head).is_some_and(|op| !op.fence()) {
        if let Some(index) = crate::threads::selector(items) {
            if items[index].atom().is_some_and(|a| a.starts_with('$')) {
                resolve_application(
                    source,
                    scopes,
                    scope,
                    SymbolKind::Memory,
                    &mut items[index],
                    &BTreeSet::new(),
                )?;
            } else if !scope.is_empty() {
                return Err(Error::at(
                    source,
                    node.span,
                    "file modules require a named memory operand",
                )
                .with_code("E_MEMORY_USE"));
            }
        } else if !scope.is_empty() && !items.iter().any(|n| n.head() == Some("atomic.at")) {
            return Err(Error::at(
                source,
                node.span,
                "file modules require a named memory operand",
            )
            .with_code("E_MEMORY_USE"));
        }
    }
    if matches!(head.as_str(), "atomic.cell" | "readonly.region") && items.len() == 5 {
        resolve_application(
            source,
            scopes,
            scope,
            SymbolKind::Memory,
            &mut items[2],
            &BTreeSet::new(),
        )?;
    }
    if head == "resource" && items.len() == 3 {
        let close = &mut items[2];
        close.kind = Kind::Atom(resolve(source, scopes, scope, SymbolKind::Function, close)?);
    }
    if head == "throws" {
        for name in &mut items[1..] {
            if name.atom() != Some("*") {
                name.kind = Kind::Atom(resolve(source, scopes, scope, SymbolKind::Tag, name)?);
            }
        }
    }
    for item in items.iter_mut().skip(usize::from(!head.is_empty())) {
        rewrite(source, scopes, scope, item)?;
    }
    Ok(())
}

fn resolve_application(
    source: &str,
    scopes: &Scopes,
    scope: &str,
    kind: SymbolKind,
    node: &mut Sexp,
    parameters: &BTreeSet<String>,
) -> Result<(), Error> {
    let name = if node.head().is_some_and(|h| h.starts_with('$')) {
        let Kind::List(items) = &mut node.kind else {
            unreachable!()
        };
        &mut items[0]
    } else {
        node
    };
    if name
        .atom()
        .is_some_and(|a| a.starts_with('$') && !parameters.contains(a))
    {
        name.kind = Kind::Atom(resolve(source, scopes, scope, kind, name)?);
        name.context.global = true;
    }
    Ok(())
}

pub(crate) struct Expanded {
    pub syntax: Sexp,
    names: BTreeMap<String, BTreeMap<String, String>>,
    opaque: BTreeMap<String, Opaque>,
    zones: Vec<(crate::ir::Span, String)>,
}
struct Opaque {
    owner: String,
    declaration: crate::ir::Span,
}
impl Expanded {
    pub fn validate_representation(
        &self,
        source: &str,
        name: &str,
        span: crate::ir::Span,
        origins: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        if let Some(opaque) = self
            .opaque
            .get(origins.get(name).map(String::as_str).unwrap_or(name))
        {
            let caller = self
                .zones
                .iter()
                .find(|(s, _)| s.start <= span.start && span.end <= s.end)
                .map(|(_, name)| name.as_str())
                .unwrap_or("");
            if caller != opaque.owner {
                let owner = if opaque.owner.is_empty() {
                    "the root module".to_owned()
                } else {
                    format!("module `${}`", opaque.owner)
                };
                return Err(Error::at(
                            source,
                            span,
                            format!("opaque type representation is private to its module: `{name}` belongs to {owner}"),
                        ).with_code("E_OPAQUE_REPRESENTATION")
                            .related(source, opaque.declaration, format!("opaque type `{name}` declared here"))
                            .help(format!("use public functions of {owner} to access this value")));
            }
        }
        Ok(())
    }
    pub fn annotate(&self, error: Error) -> Error {
        let scope = error
            .diagnostic()
            .primary
            .as_ref()
            .and_then(|label| {
                self.zones
                    .iter()
                    .find(|(span, _)| span.start <= label.span.start && label.span.start < span.end)
            })
            .map(|(_, scope)| scope.as_str())
            .unwrap_or("");
        if let Some(names) = self.names.get(scope) {
            error.rewrite_names(names)
        } else {
            error
        }
    }
    pub fn validate(
        &self,
        source: &str,
        module: &crate::ir::Module,
        origins: &BTreeMap<String, String>,
    ) -> Result<(), Error> {
        use crate::ir::{ExprKind::*, Type};
        for function in &module.functions {
            let mut pending = function.body.iter().collect::<Vec<_>>();
            while let Some(e) = pending.pop() {
                let ty = match &e.kind {
                    ResourceNew { ty, .. }
                    | Newtype { ty, .. }
                    | StructNew { ty, .. }
                    | StructGet { ty, .. }
                    | StructSet { ty, .. }
                    | ArrayNew { ty, .. }
                    | ArrayFixed { ty, .. }
                    | ArrayGet { ty, .. }
                    | ArraySet { ty, .. }
                    | VariantNew { ty, .. }
                    | Match { ty, .. } => Some(*ty),
                    ResourceHandle(object) | ArrayLen(object) => match object.ty {
                        Type::Ref { ty, .. } => Some(ty),
                        Type::Borrow(ty) => Some(ty),
                        _ => None,
                    },
                    List {
                        op: crate::ir::ListOp::FromArray(ty),
                        ..
                    } => Some(*ty),
                    Buffer {
                        op: crate::ir::BufferOp::FromArray(ty),
                        ..
                    } => Some(*ty),
                    _ => None,
                };
                if let Some(id) = ty {
                    self.validate_representation(
                        source,
                        &module.types[id.0].name,
                        e.span,
                        origins,
                    )?;
                }
                pending.extend(e.children());
            }
        }
        Ok(())
    }
}

pub(crate) fn expand(source: &str, root: &Sexp) -> Result<Expanded, Error> {
    let Some(items) = root.list().filter(|_| root.head() == Some("module")) else {
        return Ok(Expanded {
            syntax: root.clone(),
            names: BTreeMap::new(),
            opaque: BTreeMap::new(),
            zones: vec![],
        });
    };
    if !items[1..].iter().any(|n| {
        matches!(
            n.head(),
            Some(
                "namespace"
                    | "macro"
                    | "file.namespace"
                    | "use"
                    | "public"
                    | "type.alias"
                    | "export"
                    | "import"
            )
        ) || n.list().and_then(|p| p.get(2)).and_then(Sexp::head) == Some("type.params")
    }) {
        return Ok(Expanded {
            syntax: root.clone(),
            names: BTreeMap::new(),
            opaque: BTreeMap::new(),
            zones: vec![],
        });
    }
    let mut scopes = Scopes::new();
    collect(source, "", &items[1..], &mut scopes, None)?;
    resolve_imports(source, &mut scopes)?;
    let display_names = scopes
        .iter()
        .map(|(scope, symbols)| {
            let mut candidates: BTreeMap<String, Option<String>> = BTreeMap::new();
            for (symbol, concrete) in symbols.definitions.iter().chain(&symbols.aliases) {
                if concrete == &symbol.name {
                    continue;
                }
                candidates
                    .entry(concrete.clone())
                    .and_modify(|name| {
                        if name.as_ref() != Some(&symbol.name) {
                            *name = None;
                        }
                    })
                    .or_insert_with(|| Some(symbol.name.clone()));
            }
            (
                scope.clone(),
                candidates
                    .into_iter()
                    .filter_map(|(key, value)| value.map(|v| (key, v)))
                    .collect(),
            )
        })
        .collect();
    let mut output = vec![items[0].clone()];
    let opaque = scopes
        .iter()
        .flat_map(|(scope, s)| {
            s.opaque.iter().map(|symbol| {
                let declaration = s
                    .declarations
                    .iter()
                    .find(|node| {
                        matches!(node.head(), Some("type" | "type.alias" | "closure"))
                            && node
                                .list()
                                .and_then(|items| items.get(1))
                                .and_then(Sexp::atom)
                                == Some(symbol.name.as_str())
                    })
                    .expect("opaque symbol has a type declaration");
                (
                    s.definitions[symbol].clone(),
                    Opaque {
                        owner: scope.clone(),
                        declaration: declaration.span,
                    },
                )
            })
        })
        .collect();
    let mut zones = Vec::new();
    let names = scopes.keys().cloned().collect::<Vec<_>>();
    for name in names {
        let declarations = std::mem::take(&mut scopes.get_mut(&name).unwrap().declarations);
        for mut node in declarations {
            zones.push((node.span, name.clone()));
            let parameters = crate::generics::parameters(source, &node)?
                .into_iter()
                .collect();
            crate::type_syntax::map_types(&mut node, &mut |ty| {
                resolve_application(source, &scopes, &name, SymbolKind::Type, ty, &parameters)?;
                Ok(())
            })?;
            crate::type_syntax::map_references(&mut node, &mut |function, operand| {
                resolve_application(
                    source,
                    &scopes,
                    &name,
                    if function == crate::type_syntax::ReferenceKind::Function {
                        SymbolKind::Function
                    } else {
                        SymbolKind::Type
                    },
                    operand,
                    &parameters,
                )
            })?;
            rewrite(source, &scopes, &name, &mut node)?;
            output.push(node);
        }
    }
    // WAT's declarative export form can expose a selected static binding.
    // Attach it after privacy checks, so imported files cannot self-export to
    // the enclosing Wasm instance and callers need no forwarding function.
    let exports = output
        .iter()
        .filter(|n| n.head() == Some("export"))
        .cloned()
        .collect::<Vec<_>>();
    output.retain(|n| n.head() != Some("export"));
    for export in exports {
        let parts = export.list().unwrap();
        let descriptor = parts.get(2).and_then(Sexp::list).unwrap_or(&[]);
        if parts.len() != 3
            || parts[1].text().is_none()
            || descriptor.len() != 2
            || !matches!(descriptor[0].atom(), Some("func" | "memory"))
        {
            return Err(Error::at(
                source,
                export.span,
                "expected (export \"name\" (func/memory $name))",
            ));
        }
        let target = descriptor[1].atom();
        if target.is_none() {
            output.push(export);
            continue;
        }
        let definition = output
            .iter_mut()
            .find(|n| {
                let declaration = if n.head() == Some("import") {
                    n.list().and_then(|p| p.get(3)).unwrap_or(n)
                } else {
                    n
                };
                declaration.head() == descriptor[0].atom()
                    && declaration
                        .list()
                        .and_then(|p| p.get(1))
                        .and_then(Sexp::atom)
                        == target
            })
            .ok_or_else(|| Error::at(source, export.span, "exports belong to defined functions"))?;
        let definition = if definition.head() == Some("import") {
            let Kind::List(parts) = &mut definition.kind else {
                unreachable!()
            };
            &mut parts[3]
        } else {
            definition
        };
        let Kind::List(items) = &mut definition.kind else {
            unreachable!()
        };
        items.insert(
            2,
            Sexp {
                context: Default::default(),
                kind: Kind::List(vec![parts[0].clone(), parts[1].clone()]),
                span: export.span,
            },
        );
    }
    Ok(Expanded {
        names: display_names,
        syntax: Sexp {
            context: Default::default(),
            kind: Kind::List(output),
            span: root.span,
        },
        opaque,
        zones,
    })
}
