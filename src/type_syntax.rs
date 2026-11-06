//! Shared visitors for value types and nominal function / heap operands.
use crate::{
    Error,
    sexpr::{Kind, Sexp},
};

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
fn map_type(
    node: &mut Sexp,
    f: &mut impl FnMut(&mut Sexp) -> Result<(), Error>,
) -> Result<(), Error> {
    f(node)?;
    if matches!(
        node.head(),
        Some("list" | "option" | "result" | "pair" | "own" | "future" | "stream")
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
        "struct.new" | "struct.get" | "struct.set" | "closure.new" | "bytes.from-array"
        | "array.new" | "array.new_fixed" | "array.get" | "array.set" | "variant.new"
        | "ref.null" | "resource.new" | "newtype.wrap" | "newtype.unwrap" => {
            (ReferenceKind::Type, 1)
        }
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
    for child in items.iter_mut().skip(1) {
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
    node.atom().is_some()
        || node.head().is_some_and(|head| head.starts_with('$'))
        || matches!(
            node.head(),
            Some("ref" | "list" | "option" | "result" | "pair" | "own" | "future" | "stream")
        )
}

/// Apply `f` only to source type positions, never declaration names, locals,
/// labels, case names, or function operands. Invalid forms remain for the
/// ordinary frontend to diagnose.
pub(crate) fn map_types(
    node: &mut Sexp,
    f: &mut impl FnMut(&mut Sexp) -> Result<(), Error>,
) -> Result<(), Error> {
    let head = node.head().unwrap_or("").to_owned();
    let Kind::List(items) = &mut node.kind else {
        return Ok(());
    };
    match head.as_str() {
        _ if head.starts_with('@') => {}
        "type.alias" => {
            let index = definition_index(items);
            for header in items.iter_mut().take(index).skip(2) {
                map_types(header, f)?;
            }
            if let Some(target) = items.get_mut(index) {
                map_type(target, f)?;
            }
        }
        "type" => {
            let index = definition_index(items);
            for header in items.iter_mut().take(index).skip(2) {
                map_types(header, f)?;
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
                    if matches!(head.as_str(), "shared" | "nonunit") {
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
                            map_types(result, f)?;
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
        "let" => {
            let start = if items.len() == 5 && items[1].atom().is_some() {
                map_type(&mut items[2], f)?;
                3
            } else {
                2
            };
            for expression in items.iter_mut().skip(start) {
                map_types(expression, f)?;
            }
        }
        "bind" => {
            if let Some(ty) = items.get_mut(2) {
                map_type(ty, f)?;
            }
            for expression in items.iter_mut().skip(3) {
                map_types(expression, f)?;
            }
        }
        "match" => {
            if let Some(first) = items.get_mut(1) {
                if type_argument(first) {
                    map_type(first, f)?;
                } else {
                    map_types(first, f)?;
                }
            }
            for expression in items.iter_mut().skip(2) {
                map_types(expression, f)?;
            }
        }
        "case" | "catch" => {
            for expression in items.iter_mut().skip(3) {
                map_types(expression, f)?;
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
            for expression in items.iter_mut().skip(start) {
                map_types(expression, f)?;
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
            for (index, expression) in items.iter_mut().enumerate().skip(start) {
                if index != binders {
                    map_types(expression, f)?;
                }
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
                map_types(e, f)?;
            }
        }
        "func" => {
            if let Some(name) = items.get_mut(1) {
                map_arguments(name, f)?;
            }
            for child in items.iter_mut().skip(2) {
                map_types(child, f)?;
            }
        }
        _ if reference(&head).is_some() => {
            let (_, index) = reference(&head).unwrap();
            if let Some(operand) = items.get_mut(index) {
                map_arguments(operand, f)?;
            }
            for expression in items.iter_mut().skip(index + 1) {
                map_types(expression, f)?;
            }
        }
        "option.none" | "option.some" | "own.new" | "future.ready" | "future.await"
        | "future.close" | "stream.empty" | "stream.cons" | "stream.next" | "stream.close"
        | "pair.new" | "result.ok" | "result.err" => {
            let count = if matches!(head.as_str(), "pair.new" | "result.ok" | "result.err") {
                2
            } else {
                1
            };
            let explicit = items.len() > count && items[1..=count].iter().all(type_argument);
            let start = if explicit {
                for ty in &mut items[1..=count] {
                    map_type(ty, f)?;
                }
                count + 1
            } else {
                1
            };
            for expression in items.iter_mut().skip(start) {
                map_types(expression, f)?;
            }
        }
        _ => {
            for child in items.iter_mut().skip(1) {
                map_types(child, f)?;
            }
        }
    }
    Ok(())
}
