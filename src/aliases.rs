//! Transparent source type aliases. Expansion only visits type positions, so
//! aliases introduce neither nominal identities nor a second type checker.
use crate::Error;
use crate::ir::Span;
use crate::sexpr::{Kind, Sexp};
use std::collections::{BTreeMap, BTreeSet};

const MAX_DEPTH: usize = 256;
const MAX_NODES: usize = 1_000_000;

pub(crate) struct Expanded {
    pub syntax: Sexp,
    /// Expanded targets are checked even when no expression uses their aliases.
    /// They do not become declarations in the executable module.
    pub targets: Vec<Sexp>,
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
        Some("option" | "result" | "pair" | "own" | "future" | "stream")
    ) && let Kind::List(items) = &mut node.kind
    {
        for item in &mut items[1..] {
            map_type(item, f)?;
        }
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
        || matches!(
            node.head(),
            Some("ref" | "option" | "result" | "pair" | "own" | "future" | "stream")
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
        "type.alias" => {
            if let Some(target) = items.get_mut(2) {
                map_type(target, f)?;
            }
        }
        "type" => {
            if let Some(definition) = items.get_mut(2) {
                let kind = definition.head().unwrap_or("").to_owned();
                if let Kind::List(parts) = &mut definition.kind {
                    match kind.as_str() {
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

#[derive(Clone)]
struct Resolved {
    node: Sexp,
    nodes: usize,
    depth: usize,
}

struct Resolver<'a> {
    source: &'a str,
    aliases: BTreeMap<String, Sexp>,
    active: Vec<(String, Span)>,
    resolved: BTreeMap<String, Resolved>,
    cached_nodes: usize,
}
impl Resolver<'_> {
    fn error(&self, node: &Sexp, message: impl std::fmt::Display) -> Error {
        Error::at(self.source, node.span, message)
    }

    fn alias(&mut self, name: &str, depth: usize, reference: Span) -> Result<Resolved, Error> {
        let target = &self.aliases[name];
        if depth > MAX_DEPTH {
            return Err(self.error(target, "type alias expansion limit exceeded"));
        }
        if let Some(resolved) = self.resolved.get(name) {
            return Ok(resolved.clone());
        }
        if let Some(start) = self.active.iter().position(|(active, _)| active == name) {
            let cycle = &self.active[start..];
            let path = cycle
                .iter()
                .map(|(name, _)| name.as_str())
                .chain([name])
                .collect::<Vec<_>>()
                .join(" -> ");
            let mut error = Error::at(
                self.source,
                reference,
                format!("cyclic type alias `{name}`"),
            )
            .with_code("E_ALIAS_CYCLE")
            .note(format!("alias cycle: {path}"));
            for edge in cycle.windows(2) {
                error = error.related(
                    self.source,
                    edge[1].1,
                    format!("type alias `{}` depends on `{}` here", edge[0].0, edge[1].0),
                );
            }
            error = error.related(
                self.source,
                reference,
                format!(
                    "type alias `{}` depends on `{name}` here, closing the cycle",
                    cycle.last().unwrap().0
                ),
            );
            return Err(error.help("break a reference in this cycle by using the intended concrete type or a type outside this cycle; keep the required public aliases"));
        }
        self.active.push((name.to_owned(), reference));
        let target = target.clone();
        let resolved = self.ty(&target, depth + 1)?;
        self.active.pop();
        self.cached_nodes += resolved.nodes;
        if self.cached_nodes > MAX_NODES {
            return Err(self.error(&target, "type alias expansion limit exceeded"));
        }
        self.resolved.insert(name.to_owned(), resolved.clone());
        Ok(resolved)
    }

    fn ty(&mut self, node: &Sexp, depth: usize) -> Result<Resolved, Error> {
        if depth > MAX_DEPTH {
            return Err(self.error(node, "type alias expansion limit exceeded"));
        }
        if let Some(name) = node.atom().filter(|name| self.aliases.contains_key(*name)) {
            return self.alias(name, depth, node.span);
        }
        let mut output = node.clone();
        if matches!(
            node.head(),
            Some("option" | "result" | "pair" | "own" | "future" | "stream")
        ) && let Kind::List(items) = &mut output.kind
        {
            let mut nodes = 2; // This list and its constructor atom.
            let mut shape_depth = 1;
            for item in items.iter_mut().skip(1) {
                let resolved = self.ty(item, depth + 1)?;
                nodes += resolved.nodes;
                shape_depth = shape_depth.max(resolved.depth + 1);
                if nodes > MAX_NODES || shape_depth > MAX_DEPTH {
                    return Err(self.error(node, "type alias expansion limit exceeded"));
                }
                *item = resolved.node;
            }
            Ok(Resolved {
                node: output,
                nodes,
                depth: shape_depth,
            })
        } else {
            let (nodes, depth) = measure(self.source, &output)?;
            Ok(Resolved {
                node: output,
                nodes,
                depth,
            })
        }
    }
}

fn measure(source: &str, root: &Sexp) -> Result<(usize, usize), Error> {
    let mut pending = vec![(root, 0)];
    let mut nodes = 0;
    let mut maximum_depth = 0;
    while let Some((node, depth)) = pending.pop() {
        nodes += 1;
        maximum_depth = maximum_depth.max(depth);
        if nodes > MAX_NODES || depth > MAX_DEPTH {
            return Err(Error::at(
                source,
                node.span,
                "type alias expansion limit exceeded",
            ));
        }
        if let Some(items) = node.list() {
            pending.extend(items.iter().map(|item| (item, depth + 1)));
        }
    }
    Ok((nodes, maximum_depth))
}

pub(crate) fn expand(source: &str, root: &Sexp) -> Result<Expanded, Error> {
    measure(source, root)?;
    let Some(items) = root.list().filter(|_| root.head() == Some("module")) else {
        return Err(Error::at(source, root.span, "expected (module ...)"));
    };
    let mut resolver = Resolver {
        source,
        aliases: BTreeMap::new(),
        active: vec![],
        resolved: BTreeMap::new(),
        cached_nodes: 0,
    };
    let mut names = BTreeSet::new();
    for declaration in &items[1..] {
        if !matches!(declaration.head(), Some("type" | "closure" | "type.alias")) {
            continue;
        }
        let parts = declaration.list().unwrap();
        let name = parts.get(1).and_then(Sexp::atom).unwrap_or("");
        if declaration.head() == Some("type.alias") {
            if parts.len() != 3 {
                return Err(resolver.error(declaration, "expected (type.alias $name type)"));
            }
            if !name.starts_with('$')
                || name.len() == 1
                || name.starts_with("$__")
                || !name[1..]
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
            {
                return Err(resolver.error(&parts[1], "expected ASCII $identifier"));
            }
            resolver.aliases.insert(name.to_owned(), parts[2].clone());
        }
        if !names.insert(name.to_owned()) {
            return Err(resolver.error(declaration, "duplicate type declaration"));
        }
    }
    let names = resolver.aliases.keys().cloned().collect::<Vec<_>>();
    let mut targets = Vec::with_capacity(names.len());
    for name in names {
        let reference = resolver.aliases[&name].span;
        targets.push(resolver.alias(&name, 0, reference)?.node);
    }
    let mut syntax = root.clone();
    let Kind::List(items) = &mut syntax.kind else {
        unreachable!()
    };
    items.retain(|item| item.head() != Some("type.alias"));
    let (mut nodes, _) = measure(source, &syntax)?;
    map_types(&mut syntax, &mut |node| {
        if let Some(resolved) = node.atom().and_then(|name| resolver.resolved.get(name)) {
            nodes += resolved.nodes - 1;
            if nodes > MAX_NODES {
                return Err(resolver.error(node, "type alias expansion limit exceeded"));
            }
            let span = node.span;
            *node = resolved.node.clone();
            node.span = span;
        }
        Ok(())
    })?;
    measure(source, &syntax)?;
    Ok(Expanded { syntax, targets })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_positions_include_nested_payloads_but_exclude_all_other_names() {
        for (source, expected) in [
            ("(type.alias $T (pair (option $T) (ref $T)))", 1),
            ("(type $T (struct (field $T (mut $T)) (field $T)))", 2),
            ("(type $T (array (mut $T)))", 1),
            ("(type $T (variant (T $T (option $T))))", 2),
            ("(tag $T (param $T (option $T)))", 2),
            (
                "(func $T (param $T $T) (result $T) (call $T (local.get $T)))",
                2,
            ),
            (
                "(closure $T (capture $T $T) (param $T $T) (result $T) (local.get $T))",
                3,
            ),
            ("(law \"law\" (param $T $T) (i32.const 1))", 1),
            ("(let $T $T (local.get $T) (local.get $T))", 1),
            (
                "(let ($T $x) (pair.new (local.get $T) (local.get $x)) (local.get $T))",
                0,
            ),
            (
                "(loop $T (bind $T $T (local.get $T)) (result $T) (recur $T (local.get $T)))",
                2,
            ),
            (
                "(for $T (bind $T $T (local.get $T)) (while (i32.const 0)) (step (local.get $T)) (nop))",
                1,
            ),
            (
                "(match $T (result $T) (local.get $T) (case T ($T) (local.get $T)))",
                2,
            ),
            (
                "(match (let $T $T (local.get $T) (local.get $T)) (case T ($T) (local.get $T)))",
                1,
            ),
            ("(match (local.get $T) (case T ($T) (local.get $T)))", 0),
            (
                "(result.let ($T) (result $T $T) $T (local.get $T) (local.get $T))",
                3,
            ),
            ("(result.let ($T) (local.get $T) (local.get $T))", 0),
            (
                "(stream.fold-result $T $T $T (move $T) (local.get $T) ($T $x) (local.get $T))",
                3,
            ),
            (
                "(stream.fold-result (move $T) (local.get $T) ($T $x) (local.get $T))",
                0,
            ),
            ("(pair.new $T $T (local.get $T) (local.get $T))", 2),
            ("(pair.new (local.get $T) (local.get $T))", 0),
            ("(result.ok $T $T (local.get $T))", 2),
            ("(result.ok (local.get $T))", 0),
            ("(result.ok)", 0),
            ("(option.some $T (local.get $T))", 1),
            ("(option.some (local.get $T))", 0),
            ("(own.new $T (local.get $T))", 1),
            ("(future.await $T (move $T))", 1),
            ("(stream.cons $T (local.get $T) (move $T))", 1),
            (
                "(try (result $T) (do (local.get $T)) (catch $T ($T) (local.get $T)))",
                1,
            ),
            ("(struct.get $T $T (local.get $T))", 0),
            ("(ref.null $T)", 0),
        ] {
            let mut syntax = crate::sexpr::parse(source).unwrap();
            let mut count = 0;
            map_types(&mut syntax, &mut |node| {
                if node.atom() == Some("$T") {
                    count += 1;
                    node.kind = Kind::Atom("$Expanded".into());
                }
                Ok(())
            })
            .unwrap();
            assert_eq!(count, expected, "{source}");
        }
    }

    #[test]
    fn forward_aliases_expand_without_touching_local_names() {
        let source = "(module (type.alias $A (option $B)) (type.alias $B i32) (func $A (param $A $A) (result $A) (local.get $A)))";
        let syntax = crate::sexpr::parse(source).unwrap();
        let expanded = expand(source, &syntax).unwrap();
        let function = &expanded.syntax.list().unwrap()[1];
        assert_eq!(function.list().unwrap()[1].atom(), Some("$A"));
        let parameter = function.list().unwrap()[2].list().unwrap();
        assert_eq!(parameter[1].atom(), Some("$A"));
        assert_eq!(parameter[2].head(), Some("option"));
        assert_eq!(parameter[2].list().unwrap()[1].atom(), Some("i32"));
        assert_eq!(expanded.targets.len(), 2);
    }

    #[test]
    fn cyclic_and_exponentially_expanding_aliases_are_bounded() {
        let source = "(module (type.alias $A (option $B)) (type.alias $B $A))";
        assert!(
            expand(source, &crate::sexpr::parse(source).unwrap())
                .err()
                .unwrap()
                .to_string()
                .contains("cyclic")
        );
        let mut source = String::from("(module (type.alias $A0 i32)");
        for index in 1..24 {
            source.push_str(&format!(
                "(type.alias $A{index} (pair $A{} $A{}))",
                index - 1,
                index - 1
            ));
        }
        source.push(')');
        assert!(
            expand(&source, &crate::sexpr::parse(&source).unwrap())
                .err()
                .unwrap()
                .to_string()
                .contains("limit")
        );
    }
}
