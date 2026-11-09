//! Shared visitors for value types and nominal function / heap operands.
use crate::{
    Error,
    sexpr::{Kind, Sexp},
};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ReferenceKind {
    Type,
    Function,
}

pub(crate) fn definition_index(items: &[Sexp]) -> usize {
    let mut index = 2;
    if items.get(index).and_then(Sexp::head) == Some("type.params") {
        index += 1;
    }
    if items.get(index).and_then(Sexp::head) == Some("where") {
        index += 1;
    }
    index
}

/// Visit a type before its payload types. A visitor may replace the whole type.
/// Heap names inside `ref` remain nominal names and are resolved by modules.
pub(crate) fn map_type(
    node: &mut Sexp,
    f: &mut impl FnMut(&mut Sexp) -> Result<(), Error>,
) -> Result<(), Error> {
    f(node)?;
    if matches!(
        node.head(),
        Some("list" | "option" | "result" | "pair" | "own" | "future" | "stream" | "task")
    ) || node.head().is_some_and(|h| h.starts_with('$'))
    {
        if let Kind::List(items) = &mut node.kind {
            for item in &mut items[1..] {
                map_type(item, f)?;
            }
        }
    } else if matches!(node.head(), Some("ref" | "borrow" | "resource"))
        && let Kind::List(items) = &mut node.kind
        && let Some(application) = items.last_mut()
    {
        map_arguments(application, f)?;
    }
    Ok(())
}

fn map_arguments(
    node: &mut Sexp,
    f: &mut impl FnMut(&mut Sexp) -> Result<(), Error>,
) -> Result<(), Error> {
    if node.head().is_some_and(|h| h.starts_with('$'))
        && let Kind::List(items) = &mut node.kind
    {
        for ty in &mut items[1..] {
            map_type(ty, f)?;
        }
    }
    Ok(())
}

/// Named function / heap operands, separate from value type positions. Transparent
/// aliases must never be substituted into a nominal heap or function name.
fn reference(head: &str) -> Option<(ReferenceKind, usize)> {
    Some(match head {
        "call" | "closure.call" => (ReferenceKind::Function, 1),
        "task.spawn" => (ReferenceKind::Function, 2),
        "struct.new" | "struct.named" | "struct.get" | "struct.set" | "struct.update"
        | "closure.new" | "bytes.from-array" | "array.new" | "array.new_fixed" | "array.get"
        | "array.set" | "variant.new" | "ref.null" | "resource.new" | "newtype.wrap"
        | "newtype.unwrap" => (ReferenceKind::Type, 1),
        "list.from-array" => (ReferenceKind::Type, 2),
        _ => return None,
    })
}

pub(crate) fn map_references(
    node: &mut Sexp,
    f: &mut impl FnMut(ReferenceKind, &mut Sexp) -> Result<(), Error>,
) -> Result<(), Error> {
    let head = node.head().unwrap_or("").to_owned();
    if head.starts_with('@') {
        return Ok(());
    }
    let Kind::List(items) = &mut node.kind else {
        return Ok(());
    };
    if let Some((function, index)) = reference(&head) {
        if let Some(operand) = items.get_mut(index) {
            f(function, operand)?;
        }
    } else if (matches!(head.as_str(), "ref" | "borrow") || head == "resource" && items.len() == 2)
        && let Some(operand) = items.last_mut()
    {
        f(ReferenceKind::Type, operand)?;
    }
    for child in items.iter_mut().skip(usize::from(!head.is_empty())) {
        map_references(child, f)?;
    }
    Ok(())
}

fn map_storage(
    node: &mut Sexp,
    f: &mut impl FnMut(&mut Sexp) -> Result<(), Error>,
) -> Result<(), Error> {
    if node.head() == Some("mut") {
        if let Kind::List(items) = &mut node.kind
            && let Some(ty) = items.get_mut(1)
        {
            map_type(ty, f)?;
        }
        Ok(())
    } else {
        map_type(node, f)
    }
}

/// Folded value expressions have different heads from constructed types. This
/// distinguishes explicit constructor arguments from the inferred short forms.
fn type_argument(node: &Sexp) -> bool {
    node.atom().is_some_and(|name| {
        name.starts_with('$')
            || name.starts_with("@generic.")
            || crate::ir::NumType::parse(name).is_some()
            || matches!(name, "unit" | "bytes" | "text")
    }) || node.head().is_some_and(|head| head.starts_with('$'))
        || matches!(
            node.head(),
            Some(
                "ref"
                    | "resource"
                    | "borrow"
                    | "list"
                    | "option"
                    | "result"
                    | "pair"
                    | "own"
                    | "future"
                    | "stream"
                    | "task"
            )
        )
}

/// A short constructor can contain bare locals where the explicit spelling
/// contains types. Only the overlapping arities need lexical disambiguation.
pub(crate) fn explicit_constructor(
    items: &[Sexp],
    is_type: impl Fn(&Sexp) -> bool,
    is_local: impl Fn(&Sexp) -> bool,
) -> bool {
    let head = items.first().and_then(Sexp::atom).unwrap_or("");
    let count = if matches!(head, "pair.new" | "result.ok" | "result.err") {
        2
    } else {
        1
    };
    let overlap = matches!(
        (head, items.len()),
        ("pair.new", 3) | ("option.some" | "own.new", 2)
    );
    items.len() > count
        && items[1..=count]
            .iter()
            .all(|n| is_type(n) && !(overlap && is_local(n)))
}

/// Apply `f` only to source type positions, never declaration names, locals,
/// labels, case names, or function operands. Invalid forms remain for the
/// ordinary frontend to diagnose.
pub(crate) fn map_types(
    node: &mut Sexp,
    f: &mut impl FnMut(&mut Sexp) -> Result<(), Error>,
) -> Result<(), Error> {
    walk_types(node, f, &BTreeSet::new())
}

fn bind_names(node: &Sexp, locals: &mut BTreeSet<String>) {
    if node.head() == Some("@fields") {
        for field in &node.list().unwrap()[1..] {
            if let Some([_, name]) = field.list() {
                bind_names(name, locals);
            }
        }
        return;
    }
    if let Some(name) = node.atom() {
        locals.insert(name.to_owned());
    } else if let Some(names) = node.list() {
        for name in names {
            bind_names(name, locals);
        }
    }
}

fn walk_types(
    node: &mut Sexp,
    f: &mut impl FnMut(&mut Sexp) -> Result<(), Error>,
    locals: &BTreeSet<String>,
) -> Result<(), Error> {
    let head = node.head().unwrap_or("").to_owned();
    let Kind::List(items) = &mut node.kind else {
        return Ok(());
    };
    match head.as_str() {
        _ if head.starts_with('@') => {}
        "macro" => {
            for rule in items.iter_mut().skip(2) {
                let Kind::List(fields) = &mut rule.kind else {
                    continue;
                };
                let last = fields.len().saturating_sub(1);
                for clause in fields.iter_mut().take(last).skip(2) {
                    if clause.head() == Some("when")
                        && let Kind::List(guards) = &mut clause.kind
                    {
                        for guard in guards.iter_mut().skip(1) {
                            if guard.head() == Some("type")
                                && let Kind::List(parts) = &mut guard.kind
                                && parts.len() == 3
                            {
                                map_type(&mut parts[2], f)?;
                            }
                        }
                    }
                }
                if let Some(template) = fields.last_mut() {
                    walk_types(template, f, locals)?;
                }
            }
        }
        "type.alias" => {
            let index = definition_index(items);
            for header in items.iter_mut().take(index).skip(2) {
                walk_types(header, f, locals)?;
            }
            if let Some(target) = items.get_mut(index) {
                map_type(target, f)?;
            }
        }
        "type" => {
            let index = definition_index(items);
            for header in items.iter_mut().take(index).skip(2) {
                walk_types(header, f, locals)?;
            }
            if let Some(definition) = items.get_mut(index) {
                let kind = definition.head().unwrap_or("").to_owned();
                if let Kind::List(parts) = &mut definition.kind {
                    match kind.as_str() {
                        "newtype" => {
                            if let Some(base) = parts.get_mut(1) {
                                map_type(base, f)?;
                            }
                        }
                        "struct" => {
                            for field in &mut parts[1..] {
                                if field.head() == Some("field")
                                    && let Kind::List(parts) = &mut field.kind
                                    && (2..=3).contains(&parts.len())
                                {
                                    map_storage(parts.last_mut().unwrap(), f)?;
                                }
                            }
                        }
                        "array" => {
                            if let Some(storage) = parts.get_mut(1) {
                                map_storage(storage, f)?;
                            }
                        }
                        "variant" => {
                            for case in &mut parts[1..] {
                                if let Kind::List(payloads) = &mut case.kind {
                                    for payload in payloads.iter_mut().skip(1) {
                                        map_type(payload, f)?;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        "where" => {
            for clause in items.iter_mut().skip(1) {
                let head = clause.head().unwrap_or("").to_owned();
                if let Kind::List(parts) = &mut clause.kind {
                    if matches!(head.as_str(), "shared" | "nonunit")
                        || crate::typeclasses::Class::parse(&head).is_some()
                    {
                        for target in parts.iter_mut().skip(1) {
                            map_type(target, f)?;
                        }
                    } else if head == "callable" {
                        if let Some(target) = parts.get_mut(1) {
                            map_type(target, f)?;
                        }
                        if let Some(parameters) = parts.get_mut(2)
                            && let Kind::List(types) = &mut parameters.kind
                        {
                            for ty in types.iter_mut().skip(1) {
                                map_type(ty, f)?;
                            }
                        }
                        if let Some(result) = parts.get_mut(3) {
                            walk_types(result, f, locals)?;
                        }
                    }
                }
            }
        }
        "tag" => {
            for parameter in items.iter_mut().skip(2) {
                if parameter.head() == Some("param")
                    && let Kind::List(parts) = &mut parameter.kind
                {
                    for payload in &mut parts[1..] {
                        map_type(payload, f)?;
                    }
                }
            }
        }
        "param" | "capture" => {
            if let Some(ty) = items.get_mut(2) {
                map_type(ty, f)?;
            }
        }
        "result" if items.len() == 2 => map_type(&mut items[1], f)?,
        "let" if items.len() == 3 && items[1].list().is_some() => {
            let mut nested = locals.clone();
            if let Kind::List(group) = &mut items[1].kind {
                for binding in group {
                    if let Kind::List(p) = &mut binding.kind
                        && (2..=3).contains(&p.len())
                    {
                        if p.len() == 3 {
                            if p[1].head() == Some("type")
                                && let Kind::List(annotation) = &mut p[1].kind
                                && annotation.len() == 2
                            {
                                map_type(&mut annotation[1], f)?;
                            } else {
                                map_type(&mut p[1], f)?;
                            }
                        }
                        let last = p.len() - 1;
                        walk_types(&mut p[last], f, &nested)?;
                        bind_names(&p[0], &mut nested);
                    }
                }
            }
            walk_types(&mut items[2], f, &nested)?;
        }
        "let" | "using" | "for-each" => {
            let start = if items.len() == 5 && items[1].atom().is_some() {
                map_type(&mut items[2], f)?;
                3
            } else {
                2
            };
            if let Some(initial) = items.get_mut(start) {
                walk_types(initial, f, locals)?;
            }
            let mut nested = locals.clone();
            if let Some(names) = items.get(1) {
                bind_names(names, &mut nested);
            }
            for expression in items.iter_mut().skip(start + 1) {
                walk_types(expression, f, &nested)?;
            }
        }
        "bind" => {
            if let Some(ty) = items.get_mut(2) {
                map_type(ty, f)?;
            }
            for expression in items.iter_mut().skip(3) {
                walk_types(expression, f, locals)?;
            }
        }
        "struct.update" => {
            if let Some(ty) = items.get_mut(1) {
                map_arguments(ty, f)?;
            }
            if let Some(subject) = items.get_mut(2) {
                walk_types(subject, f, locals)?;
            }
            if let Some(binding) = items.get_mut(3).and_then(|n| {
                if let Kind::List(parts) = &mut n.kind {
                    Some(parts)
                } else {
                    None
                }
            }) {
                let mut nested = locals.clone();
                if let Some(name) = binding.get(1) {
                    bind_names(name, &mut nested);
                }
                for expression in binding.iter_mut().skip(2) {
                    walk_types(expression, f, &nested)?;
                }
            }
        }
        "match" => {
            let inferred = items.get(2).and_then(Sexp::head) == Some("case");
            if let Some(first) = items.get_mut(1) {
                if !inferred && type_argument(first) {
                    map_type(first, f)?;
                } else {
                    walk_types(first, f, locals)?;
                }
            }
            for expression in items.iter_mut().skip(2) {
                walk_types(expression, f, locals)?;
            }
        }
        "case" | "catch" => {
            let mut nested = locals.clone();
            if let Some(names) = items.get(2) {
                bind_names(names, &mut nested);
            }
            for expression in items.iter_mut().skip(3) {
                walk_types(expression, f, &nested)?;
            }
        }
        "catch_all" => {
            let mut nested = locals.clone();
            if let Some(names) = items.get(1) {
                bind_names(names, &mut nested);
            }
            for expression in items.iter_mut().skip(2) {
                walk_types(expression, f, &nested)?;
            }
        }
        "result.let" => {
            let start = if items.len() == 6 {
                map_type(&mut items[2], f)?;
                map_type(&mut items[3], f)?;
                4
            } else {
                2
            };
            if let Some(subject) = items.get_mut(start) {
                walk_types(subject, f, locals)?;
            }
            let mut nested = locals.clone();
            if let Some(names) = items.get(1) {
                bind_names(names, &mut nested);
            }
            for expression in items.iter_mut().skip(start + 1) {
                walk_types(expression, f, &nested)?;
            }
        }
        "result.do" => {
            let mut nested = locals.clone();
            let last = items.len().saturating_sub(1);
            for (index, child) in items.iter_mut().enumerate().skip(1) {
                if index == last {
                    walk_types(child, f, &nested)?;
                } else if let Kind::List(binding) = &mut child.kind {
                    for subject in binding.iter_mut().skip(1) {
                        walk_types(subject, f, &nested)?;
                    }
                    if let Some(name) = binding.first() {
                        bind_names(name, &mut nested);
                    }
                }
            }
        }
        "stream.fold-result" => {
            let (start, binders) = if items.len() == 8 {
                for ty in &mut items[1..4] {
                    map_type(ty, f)?;
                }
                (4, 6)
            } else {
                (1, 3)
            };
            let mut nested = locals.clone();
            if let Some(names) = items.get(binders) {
                bind_names(names, &mut nested);
            }
            for (index, expression) in items.iter_mut().enumerate().skip(start) {
                if index != binders {
                    walk_types(
                        expression,
                        f,
                        if index > binders { &nested } else { locals },
                    )?;
                }
            }
        }
        "task.spawn" | "task.join" | "task.drop" | "task.cancel" | "task.join-timeout" => {
            if let Some(ty) = items.get_mut(1) {
                map_type(ty, f)?;
            }
            let start = if head == "task.spawn" {
                if let Some(target) = items.get_mut(2) {
                    map_arguments(target, f)?;
                }
                3
            } else {
                2
            };
            for child in items.iter_mut().skip(start) {
                walk_types(child, f, locals)?;
            }
        }
        "list.new" | "list.len" | "list.get" | "list.slice" | "list.from-array" => {
            if let Some(ty) = items.get_mut(1) {
                map_type(ty, f)?;
            }
            let start = if head == "list.from-array" { 3 } else { 2 };
            if start == 3
                && let Some(array) = items.get_mut(2)
            {
                map_arguments(array, f)?;
            }
            for e in items.iter_mut().skip(start) {
                walk_types(e, f, locals)?;
            }
        }
        "func" | "closure" | "law" | "bench" => {
            if let Some(name) = items.get_mut(1) {
                map_arguments(name, f)?;
            }
            // Closure captures are explicit; never inherit another function's
            // locals while deciding whether an atom occupies a value position.
            let mut nested = BTreeSet::new();
            for child in items.iter().skip(2) {
                if matches!(child.head(), Some("param" | "capture"))
                    && let Some(name) = child.list().and_then(|p| p.get(1))
                {
                    bind_names(name, &mut nested);
                }
            }
            for child in items.iter_mut().skip(2) {
                walk_types(child, f, &nested)?;
            }
        }
        "loop" | "native.loop" | "for" => {
            let mut nested = locals.clone();
            for child in items
                .iter()
                .skip(2)
                .take_while(|n| n.head() == Some("bind"))
            {
                if let Some(name) = child.list().and_then(|p| p.get(1)) {
                    bind_names(name, &mut nested);
                }
            }
            for child in items.iter_mut().skip(1) {
                walk_types(
                    child,
                    f,
                    if child.head() == Some("bind") {
                        locals
                    } else {
                        &nested
                    },
                )?;
            }
        }
        _ if reference(&head).is_some() => {
            let (_, index) = reference(&head).unwrap();
            if let Some(operand) = items.get_mut(index) {
                map_arguments(operand, f)?;
            }
            for expression in items.iter_mut().skip(index + 1) {
                walk_types(expression, f, locals)?;
            }
        }
        "option.none" | "option.some" | "own.new" | "future.ready" | "future.await"
        | "future.close" | "stream.empty" | "stream.cons" | "stream.next" | "stream.read-batch"
        | "stream.close" | "pair.new" | "result.ok" | "result.err" => {
            let count = if matches!(head.as_str(), "pair.new" | "result.ok" | "result.err") {
                2
            } else {
                1
            };
            let explicit = explicit_constructor(items, type_argument, |n| {
                n.atom().is_some_and(|name| locals.contains(name))
            });
            let start = if explicit {
                for ty in &mut items[1..=count] {
                    map_type(ty, f)?;
                }
                count + 1
            } else {
                1
            };
            for expression in items.iter_mut().skip(start) {
                walk_types(expression, f, locals)?;
            }
        }
        _ => {
            for child in items.iter_mut().skip(usize::from(!head.is_empty())) {
                walk_types(child, f, locals)?;
            }
        }
    }
    Ok(())
}
