//! An optional, reversible Watz view of canonical Walz source.
use crate::{
    Error,
    ir::Span,
    sexpr::{Kind, Sexp},
};

mod parser;
mod printer;
#[cfg(test)]
mod tests;

/// Files using the alternate review syntax. Canonical sources use `.walz`.
pub fn is_source_path(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|ext| ext == "watz")
}

/// Render original source forms without macro expansion or optimization.
pub fn to_watz(source: &str) -> Result<String, crate::Error> {
    printer::render(source)
}

/// Lowered canonical source and its mapping back to the review input.
#[derive(Clone)]
pub struct Translation {
    pub text: String,
    pub original: String,
    mappings: Vec<(Span, Span)>,
}
impl Translation {
    pub fn original_span(&self, span: Span) -> Span {
        let point = |offset: usize, end: bool| {
            self.mappings
                .iter()
                .find(|(generated, _)| generated.start <= offset && offset < generated.end)
                .or_else(|| {
                    self.mappings
                        .last()
                        .filter(|(generated, _)| offset == generated.end)
                })
                .map(|(_, original)| if end { original.end } else { original.start })
                .unwrap_or(self.original.len())
        };
        let start = point(span.start, false);
        let end = point(span.end.saturating_sub(1).max(span.start), true).max(start);
        Span { start, end }
    }
    pub(crate) fn offsets(&self, offset: usize) -> Vec<usize> {
        self.mappings
            .iter()
            .filter(|(_, input)| input.start <= offset && offset < input.end)
            .map(|(output, _)| output.start)
            .collect()
    }
}

fn fail(source: &str, span: Span, message: &str) -> Error {
    Error::at(source, span, message).with_code("E_REVIEW_SYNTAX")
}
fn atom(name: impl Into<String>, span: Span) -> Sexp {
    Sexp {
        kind: Kind::Atom(name.into()),
        span,
        context: Default::default(),
    }
}
fn list(head: &str, args: Vec<Sexp>, span: Span) -> Sexp {
    let mut items = vec![atom(
        head,
        Span {
            start: span.start,
            end: span.start,
        },
    )];
    items.extend(args);
    Sexp {
        kind: Kind::List(items),
        span,
        context: Default::default(),
    }
}
fn identifier(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '.'))
        && !name.as_bytes()[0].is_ascii_digit()
}
fn simple_name(name: &str) -> bool {
    identifier(name) && !name.contains('.')
}
fn variant_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}
fn number(name: &str) -> bool {
    !name.contains('+')
        && (name.starts_with("0x")
            || name.starts_with("-0x")
            || name.parse::<f64>().is_ok()
                && name
                    .trim_start_matches('-')
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_digit))
}
fn string(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}
fn binary(name: &str) -> Option<&str> {
    Some(match name {
        "=" => "===",
        "!=" => "!==",
        "+" | "-" | "*" | "/" | "<" | "<=" | ">" | ">=" | "&&" | "||" | "&" | "|" | "^" | "<<"
        | ">>" | ">>>" => name,
        _ => return None,
    })
}

fn wat_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                for byte in c.to_string().as_bytes() {
                    out.push_str(&format!("\\{byte:02x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Lower an optional Watz view to Walz while retaining original source positions.
pub fn from_watz(source: &str) -> Result<Translation, Error> {
    if source.len() > 16 * 1024 * 1024 {
        return Err(fail(
            source,
            Span { start: 0, end: 0 },
            "review source exceeds 16 MiB",
        ));
    }
    let (root, comments) = parser::parse(source)?;
    // Function-tail let chains and generic types also add canonical nesting.
    // Check it before recursive serialization, using the S-expression limit.
    let mut pending = vec![(&root, 0)];
    while let Some((node, depth)) = pending.pop() {
        if depth > 256 {
            return Err(fail(
                source,
                node.span,
                "lowered review nesting limit exceeded",
            ));
        }
        if let Some(items) = node.list() {
            pending.extend(items.iter().map(|item| (item, depth + 1)));
        }
    }
    fn write(n: &Sexp, text: &mut String, mappings: &mut Vec<(Span, Span)>) {
        let start = text.len();
        match &n.kind {
            Kind::Atom(s) => text.push_str(s),
            Kind::Text(s) => text.push_str(&wat_string(s)),
            Kind::Bytes(bytes) => {
                text.push('"');
                for byte in bytes {
                    text.push_str(&format!("\\{byte:02x}"));
                }
                text.push('"');
            }
            Kind::List(p) => {
                text.push('(');
                mappings.push((
                    Span {
                        start,
                        end: text.len(),
                    },
                    Span {
                        start: n.span.start,
                        end: n.span.start,
                    },
                ));
                for (i, child) in p.iter().enumerate() {
                    if i > 0 {
                        text.push(' ');
                    }
                    write(child, text, mappings);
                }
                let start = text.len();
                text.push(')');
                mappings.push((
                    Span {
                        start,
                        end: text.len(),
                    },
                    Span {
                        start: n.span.end,
                        end: n.span.end,
                    },
                ));
                return;
            }
        }
        mappings.push((
            Span {
                start,
                end: text.len(),
            },
            n.span,
        ));
    }
    let mut text = String::new();
    for note in comments {
        for line in note.split(['\n', '\r']) {
            text.push_str(&format!(";; @review-note {line}\n"));
        }
    }
    let mut mappings = Vec::new();
    write(&root, &mut text, &mut mappings);
    if text.len() > 16 * 1024 * 1024 {
        return Err(fail(
            source,
            root.span,
            "lowered review source exceeds 16 MiB",
        ));
    }
    Ok(Translation {
        text,
        mappings,
        original: source.into(),
    })
}
