use super::*;

pub(super) fn expression(node: &Sexp) -> Option<String> {
    match node.head()? {
        "if" => conditional(node),
        "@match" => matching(node),
        _ => None,
    }
}

fn condition(node: &Sexp) -> String {
    let text = super::expression(node);
    if node
        .list()
        .is_some_and(|p| p.len() == 3 && p[0].atom().and_then(binary).is_some())
    {
        text
    } else {
        format!("({text})")
    }
}

fn branch<'a>(node: &'a Sexp, head: &str) -> Option<Option<&'a Sexp>> {
    if node.head() != Some(head) {
        return None;
    }
    Some(match node.list()? {
        [_] => None,
        [_, body] => Some(body),
        _ => return None,
    })
}

fn braced(body: Option<&Sexp>) -> String {
    body.map(|n| format!("{{\n  {}\n}}", indent_tail(&super::expression(n), 2)))
        .unwrap_or_else(|| "{}".into())
}

fn conditional(node: &Sexp) -> Option<String> {
    let p = node.list()?;
    let (results, start) = if p.get(1)?.head() == Some("result") {
        (Some(&p[1].list()?[1..]), 2)
    } else {
        (None, 1)
    };
    let [test, yes, rest @ ..] = p.get(start..)? else {
        return None;
    };
    if rest.len() > 1 {
        return None;
    }
    // A leading $name without a result header can be a native WAT label whose
    // condition comes from the stack. Avoid presenting it as a local read.
    if start == 1 && test.atom().is_some_and(|s| s.starts_with('$')) {
        return None;
    }
    let yes = branch(yes, "then")?;
    let no = if let Some(no) = rest.first() {
        Some(branch(no, "else")?)
    } else {
        None
    };
    // Validate the complete shape before rendering children: speculative
    // rendering followed by structural fallback can multiply nested work.
    let header = results
        .map(|types| format!("<{}>", types.iter().map(ty).collect::<Vec<_>>().join(", ")))
        .unwrap_or_default();
    let mut out = format!("if{header} {} {}", condition(test), braced(yes));
    if let Some(body) = no {
        if let Some(text) = body
            .filter(|n| n.head() == Some("if"))
            .and_then(conditional)
        {
            out.push_str(&format!(" else {text}"));
        } else {
            out.push_str(&format!(" else {}", braced(body)));
        }
    }
    Some(out)
}

fn matching(node: &Sexp) -> Option<String> {
    let p = node.list()?;
    let (variant, result, subject) = if matches!(p.get(2)?.head(), Some("@case" | "@default")) {
        (None, None, 1)
    } else {
        let variant = p.get(1)?;
        if p.get(2)?.head() == Some("result") {
            let [_, result] = p[2].list()? else {
                return None;
            };
            (Some(variant), Some(result), 3)
        } else {
            (Some(variant), None, 2)
        }
    };
    let clauses = p.get(subject + 1..)?;
    if clauses.is_empty() {
        return None;
    }
    enum Arm<'a> {
        Default(&'a Sexp),
        Case(&'a str, &'a [Sexp], &'a Sexp),
    }
    let mut arms = Vec::new();
    for clause in clauses {
        let c = clause.list()?;
        arms.push(match (clause.head()?, c) {
            ("@default", [_, body]) => Arm::Default(body),
            ("@case", [_, tag, names, body])
                if tag.atom().is_some_and(variant_name)
                    && names.list()?.iter().all(|n| n.atom().is_some()) =>
            {
                Arm::Case(tag.atom()?, names.list()?, body)
            }
            _ => return None,
        });
    }
    let header = variant
        .map(|v| {
            format!(
                "<{}{}>",
                ty(v),
                result.map(|r| format!(", {}", ty(r))).unwrap_or_default()
            )
        })
        .unwrap_or_default();
    let mut out = format!("match{header} {} {{\n", condition(p.get(subject)?));
    for arm in arms {
        let text = match arm {
            Arm::Default(body) => format!("default => {}", super::expression(body)),
            Arm::Case(tag, names, body) => format!(
                "case {tag}({}) => {}",
                names
                    .iter()
                    .map(super::expression)
                    .collect::<Vec<_>>()
                    .join(", "),
                super::expression(body)
            ),
        };
        out.push_str(&format!("  {},\n", indent_tail(&text, 2)));
    }
    out.push('}');
    Some(out)
}
