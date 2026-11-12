use super::*;
mod control;

fn indent_tail(text: &str, indent: usize) -> String {
    text.replace('\n', &format!("\n{}", " ".repeat(indent)))
}

fn group(open: &str, args: Vec<String>, close: &str) -> String {
    let flat = format!("{open}{}{close}", args.join(", "));
    if flat.len() <= 88 && !flat.contains('\n') {
        return flat;
    }
    format!(
        "{open}\n  {}\n{close}",
        args.iter()
            .map(|s| indent_tail(s, 2))
            .collect::<Vec<_>>()
            .join(",\n  ")
    )
}

fn expression(n: &Sexp) -> String {
    match &n.kind {
        Kind::Text(s) => string(s),
        Kind::Bytes(bytes) => format!(
            "sexpr.bytes({})",
            string(&bytes.iter().map(|b| format!("{b:02x}")).collect::<String>())
        ),
        Kind::Atom(s)
            if (identifier(s) || number(s))
                && !s.starts_with("wat.")
                && !s.starts_with("walz.")
                && !matches!(s.as_str(), "if" | "match") =>
        {
            s.clone()
        }
        Kind::Atom(s) if s.starts_with('@') && identifier(&s[1..]) => format!("walz.{}", &s[1..]),
        Kind::Atom(s) => format!("sexpr.atom({})", string(s)),
        Kind::List(p) => {
            if let Some(text) = control::expression(n) {
                return text;
            }
            if let [head, left, right] = p.as_slice()
                && let Some(op) = head.atom().and_then(binary)
            {
                let left = expression(left);
                let right = expression(right);
                let flat = format!("({left} {op} {right})");
                return if flat.len() <= 88 && !flat.contains('\n') {
                    flat
                } else {
                    format!(
                        "(\n  {}\n  {op} {}\n)",
                        indent_tail(&left, 2),
                        indent_tail(&right, 2)
                    )
                };
            }
            if let [head, target, args @ ..] = p.as_slice()
                && head.atom() == Some("call")
                && target
                    .atom()
                    .is_some_and(|s| s.starts_with('$') && identifier(s))
            {
                return group(
                    &format!("{}(", target.atom().unwrap()),
                    args.iter().map(expression).collect(),
                    ")",
                );
            }
            let Some(head) = p.first().and_then(Sexp::atom) else {
                return group("[", p.iter().map(expression).collect(), "]");
            };
            if head.starts_with('$') {
                return group("[", p.iter().map(expression).collect(), "]");
            }
            let mut args = p[1..].iter().map(expression).collect::<Vec<_>>();
            if let Some(name) = head.strip_prefix('@').filter(|s| identifier(s)) {
                group(&format!("walz.{name}("), args, ")")
            } else if identifier(head)
                && !head.starts_with("sexpr.")
                && !head.starts_with("walz.")
                && !head.starts_with("wat.")
            {
                group(&format!("wat.{head}("), args, ")")
            } else {
                args.insert(0, string(head));
                group("sexpr.form(", args, ")")
            }
        }
    }
}

fn ty(n: &Sexp) -> String {
    if let Some(p) = n.list() {
        if n.head() == Some("@inst") && p.len() >= 3 && p[1].atom().is_some_and(identifier) {
            return format!(
                "{}<{}>",
                p[1].atom().unwrap(),
                p[2..].iter().map(ty).collect::<Vec<_>>().join(", ")
            );
        }
        if let [_, target] = p
            && n.head() == Some("ref")
        {
            return format!("ref<{}>", expression(target));
        }
        if let [_, null, target] = p
            && n.head() == Some("ref")
            && null.atom() == Some("null")
        {
            return format!("refNull<{}>", expression(target));
        }
    }
    expression(n)
}

fn binding(n: &Sexp) -> Option<String> {
    let p = n.list()?;
    let name = p
        .first()?
        .atom()
        .filter(|s| s.starts_with('$') && simple_name(s))?;
    Some(match p {
        [_, value] => format!("{name} = {}", expression(value)),
        [_, annotation, value]
            if annotation.head() == Some("type") && annotation.list()?.len() == 2 =>
        {
            format!(
                "{name}: {} = {}",
                ty(&annotation.list()?[1]),
                expression(value)
            )
        }
        _ => return None,
    })
}

fn body(n: &Sexp, indent: usize, scope: &std::collections::BTreeSet<String>, output: &mut String) {
    let pad = " ".repeat(indent);
    if n.head() == Some("@let")
        && let Some(p) = n.list()
    {
        let group = match p {
            [_, name, value, _] if name.atom().is_some_and(|s| s.starts_with('$')) => {
                let b = Sexp {
                    kind: Kind::List(vec![name.clone(), value.clone()]),
                    ..n.clone()
                };
                binding(&b)
            }
            [_, name, annotation, value, _] if annotation.head() == Some("type") => {
                let b = Sexp {
                    kind: Kind::List(vec![name.clone(), annotation.clone(), value.clone()]),
                    ..n.clone()
                };
                binding(&b)
            }
            [_, bindings, _] if bindings.list().is_some_and(|p| p.len() >= 2) => bindings
                .list()
                .unwrap()
                .iter()
                .map(binding)
                .collect::<Option<Vec<_>>>()
                .map(|p| p.join(", ")),
            _ => None,
        };
        if let Some(group) = group {
            let names = if let Some(name) = p[1].atom() {
                vec![name.to_owned()]
            } else {
                p[1].list()
                    .unwrap()
                    .iter()
                    .filter_map(|n| n.list()?.first()?.atom().map(str::to_owned))
                    .collect()
            };
            let shadow = names.iter().any(|name| scope.contains(name));
            if shadow {
                // A JS let initializer has a temporal dead zone, while a
                // Walz let initializer can read the outer binding. Keep the
                // explicit form instead of suggesting JS shadowing semantics.
                output.push_str(&format!(
                    "{pad}return {};\n",
                    indent_tail(&expression(n), indent)
                ));
                return;
            }
            let mut nested = scope.clone();
            nested.extend(names);
            output.push_str(&format!("{pad}let {};\n", indent_tail(&group, indent)));
            body(p.last().unwrap(), indent, &nested, output);
            return;
        }
    }
    output.push_str(&format!(
        "{pad}return {};\n",
        indent_tail(&expression(n), indent)
    ));
}

fn declaration(n: &Sexp) -> Option<String> {
    let p = n.list()?;
    if matches!(n.head(), Some("@use" | "@export")) {
        let importing = n.head() == Some("@use");
        let start = if importing { 2 } else { 1 };
        let mut members = Vec::new();
        for member in p.get(start..)? {
            let [kind, name, rest @ ..] = member.list()? else {
                return None;
            };
            let kind = kind.atom()?;
            if !matches!(kind, "type" | "func")
                || rest.len() > 1
                || !name
                    .atom()
                    .is_some_and(|s| s.starts_with('$') && identifier(s))
            {
                return None;
            }
            let mut text = format!(
                "{}{}",
                if kind == "type" { "type " } else { "" },
                name.atom()?
            );
            if let Some(alias) = rest.first() {
                let alias = alias
                    .atom()
                    .filter(|s| s.starts_with('$') && identifier(s))?;
                text.push_str(&format!(" as {alias}"));
            }
            members.push(text);
        }
        return if importing {
            Some(format!(
                "import {{ {} }} from {};\n",
                members.join(", "),
                string(p.get(1)?.text()?)
            ))
        } else {
            Some(format!("export {{ {} }};\n", members.join(", ")))
        };
    }
    if n.head() != Some("func") {
        return None;
    }
    let name = p
        .get(1)?
        .atom()
        .filter(|s| s.starts_with('$') && simple_name(s))?;
    let mut at = 2;
    let mut decorators = String::new();
    while p.get(at)?.head() == Some("export") {
        let [_, value] = p[at].list()? else {
            return None;
        };
        decorators.push_str(&format!("@export({})\n", expression(value)));
        at += 1;
    }
    let mut parameters = Vec::new();
    while p.get(at)?.head() == Some("param") {
        let [_, name, value] = p[at].list()? else {
            return None;
        };
        let name = name
            .atom()
            .filter(|s| s.starts_with('$') && simple_name(s))?;
        parameters.push(format!("{name}: {}", ty(value)));
        at += 1;
    }
    let result = if p.get(at)?.head() == Some("result") {
        let [_, value] = p[at].list()? else {
            return None;
        };
        at += 1;
        format!(": {}", ty(value))
    } else {
        String::new()
    };
    if p.get(at)?.head() == Some("@effect") {
        decorators.push_str(&format!(
            "@effect({})\n",
            p[at].list()?[1..]
                .iter()
                .map(expression)
                .collect::<Vec<_>>()
                .join(", ")
        ));
        at += 1;
    }
    if at + 1 != p.len() {
        return None;
    }
    let mut out = format!(
        "{decorators}func {name}({}){result} {{\n",
        parameters.join(", ")
    );
    let parameters = p
        .iter()
        .filter(|n| n.head() == Some("param"))
        .filter_map(|n| n.list()?.get(1)?.atom().map(str::to_owned))
        .collect();
    body(&p[at], 2, &parameters, &mut out);
    out.push_str("}\n");
    Some(out)
}

/// Render source forms, not expanded or optimized implementation IR.
pub(super) fn render(source: &str) -> Result<String, Error> {
    if source.len() > 16 * 1024 * 1024 {
        return Err(fail(
            source,
            Span { start: 0, end: 0 },
            "review source exceeds 16 MiB",
        ));
    }
    let root = crate::sexpr::parse(source)?;
    if root.head() != Some("module") {
        return Err(fail(source, root.span, "expected a Walz module"));
    }
    let mut out =
        String::from("// Walz review syntax: structural notation, not JavaScript execution.\n");
    // Keep source comments verbatim with their original line numbers. They are
    // review context rather than executable forms or hidden parser directives.
    let lexer = wast::lexer::Lexer::new(source);
    let mut offset = 0;
    let mut line = 1;
    while let Ok(Some(token)) = lexer.parse(&mut offset) {
        let token_text = &source[token.offset..token.offset + token.len as usize];
        if matches!(
            token.kind,
            wast::lexer::TokenKind::LineComment | wast::lexer::TokenKind::BlockComment
        ) {
            for (row, text) in token_text.lines().enumerate() {
                if let Some(note) = text.strip_prefix(";; @review-note ") {
                    out.push_str(&format!("// {note}\n"));
                } else {
                    out.push_str(&format!("// source:{} {text}\n", line + row));
                }
            }
        }
        line += token_text.bytes().filter(|b| *b == b'\n').count();
    }
    for n in &root.list().unwrap()[1..] {
        if let Some(text) = declaration(n) {
            out.push_str(&text);
        } else {
            out.push_str(&format!("{};\n", expression(n)));
        }
    }
    Ok(out)
}
