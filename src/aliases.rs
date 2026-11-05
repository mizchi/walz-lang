//! Transparent source type aliases. Expansion only visits type positions, so
//! aliases introduce neither nominal identities nor a second type checker.
use crate::ir::Span;
use crate::sexpr::{Kind, Sexp};
use crate::{Error, type_syntax::map_types};
use std::collections::{BTreeMap, BTreeSet};

const MAX_DEPTH: usize = 256;
const MAX_NODES: usize = 1_000_000;

pub(crate) struct Expanded {
    pub syntax: Sexp,
    /// Expanded targets are checked even when no expression uses their aliases.
    /// They do not become declarations in the executable module.
    pub targets: Vec<Sexp>,
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
            Some("list" | "option" | "result" | "pair" | "own" | "future" | "stream")
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
