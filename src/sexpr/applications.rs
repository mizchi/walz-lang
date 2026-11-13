//! Contextual angle applications. Lexing retains one source atom, then only type
//! and nominal reference positions expand to public @inst syntax. Literal names
//! declared/imported in that namespace win; binders, locals and metadata stay data.
use super::{Kind, Sexp, SyntaxContext};
use crate::{Error, ir::Span};
use std::collections::BTreeSet;

pub(crate) fn prefix(text: &str) -> Option<usize> {
    let text = text.strip_prefix('$')?;
    let n = text
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
        .count();
    (n != 0 && text.as_bytes().get(n) == Some(&b'<')).then_some(n + 1)
}

/// Extent shared with the token-preserving formatter. Strings and comments use
/// WAT lexing; commas have meaning only inside this adjacent `$Name<...>` atom.
pub(crate) fn end(source: &str, start: usize) -> Option<usize> {
    scan(source, start, false)
}

pub(crate) fn compact_end(source: &str, start: usize) -> Option<usize> {
    source.get(start..)?.starts_with("@$").then_some(())?;
    let end = scan(source, start + 1, true)?;
    source[end..].starts_with("::").then_some(end + 2)
}

fn scan(source: &str, start: usize, compact: bool) -> Option<usize> {
    let open = start + prefix(source.get(start..)?)?;
    let (mut at, mut angles, mut parens, mut comma) = (open + 1, 1usize, 0usize, false);
    let bytes = source.as_bytes();
    while at < bytes.len() {
        if source[at..].starts_with(";;")
            || source[at..].starts_with("(;")
            || bytes[at] == b'"'
            || source[at..].starts_with("$\"")
        {
            wast::lexer::Lexer::new(source).parse(&mut at).ok()??;
            continue;
        }
        match bytes[at] {
            b'<' => angles += 1,
            b'>' if angles == 1 && parens != 0 => {}
            b'>' => {
                angles -= 1;
                if angles == 0 {
                    let end = at + 1;
                    let boundary = compact && source[end..].starts_with("::")
                        || bytes
                            .get(end)
                            .is_none_or(|b| b.is_ascii_whitespace() || b"(),{};".contains(b));
                    if !boundary {
                        return None;
                    }
                    // A valid WAT name may end in `<` and be followed by stack
                    // instructions containing another identifier ending in `>`.
                    // Never consume that surrounding code as one application.
                    if !comma
                        && parse(
                            source,
                            &atom(&source[start..end], Span { start, end }),
                            &BTreeSet::new(),
                        )
                        .is_err()
                    {
                        return None;
                    }
                    return Some(end);
                }
            }
            b'(' => parens += 1,
            b')' if parens == 0 => return comma.then_some(at),
            b')' => parens -= 1,
            b',' => comma = true,
            _ => {}
        }
        at += source[at..].chars().next()?.len_utf8();
    }
    comma.then_some(at)
}

fn atom(text: &str, span: Span) -> Sexp {
    Sexp {
        kind: Kind::Atom(text.into()),
        span,
        context: SyntaxContext::default(),
    }
}
struct Parser<'a> {
    source: &'a str,
    text: &'a str,
    base: usize,
    at: usize,
    literal_types: &'a BTreeSet<String>,
}
impl Parser<'_> {
    fn error(&self, message: &str) -> Error {
        Error::at(
            self.source,
            Span {
                start: self.base + self.at,
                end: self.base + self.at,
            },
            message,
        )
        .with_code("E_GENERIC_SYNTAX")
    }
    fn space(&mut self) -> Result<(), Error> {
        loop {
            if self.text[self.at..].starts_with(";;") || self.text[self.at..].starts_with("(;") {
                wast::lexer::Lexer::new(self.text)
                    .parse(&mut self.at)
                    .map_err(|_| self.error("unterminated comment in type arguments"))?;
            } else if let Some(c) = self.text[self.at..]
                .chars()
                .next()
                .filter(|c| c.is_whitespace())
            {
                self.at += c.len_utf8();
            } else {
                return Ok(());
            }
        }
    }
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }
    fn read(&mut self, depth: usize) -> Result<Sexp, Error> {
        if depth > 256 {
            return Err(self.error("type argument nesting limit exceeded"));
        }
        self.space()?;
        let start = self.at;
        // Compile-time type expressions such as @component.type carry WAT
        // strings as selector data. The enclosing type checker still rejects
        // strings as bare type arguments.
        if self.peek() == Some(b'"') {
            let token = wast::lexer::Lexer::new(self.text)
                .parse(&mut self.at)
                .map_err(|_| self.error("invalid string in type expression"))?
                .ok_or_else(|| self.error("expected string in type expression"))?;
            if token.kind != wast::lexer::TokenKind::String {
                return Err(self.error("expected a separate string literal in type expression"));
            }
            let bytes = token.string(self.text).into_owned();
            return Ok(Sexp {
                kind: match String::from_utf8(bytes) {
                    Ok(s) => Kind::Text(s),
                    Err(e) => Kind::Bytes(e.into_bytes()),
                },
                span: Span {
                    start: self.base + start,
                    end: self.base + self.at,
                },
                context: SyntaxContext::default(),
            });
        }
        if self.peek() == Some(b'(') {
            self.at += 1;
            let mut items = Vec::new();
            loop {
                self.space()?;
                if self.peek() == Some(b')') {
                    self.at += 1;
                    break;
                }
                if self.peek().is_none() {
                    return Err(self.error("expected ')' in type argument"));
                }
                items.push(self.read(depth + 1)?);
            }
            return Ok(Sexp {
                kind: Kind::List(items),
                span: Span {
                    start: self.base + start,
                    end: self.base + self.at,
                },
                context: SyntaxContext::default(),
            });
        }
        if self.text[self.at..].starts_with("$\"") {
            wast::lexer::Lexer::new(self.text)
                .parse(&mut self.at)
                .map_err(|_| self.error("invalid quoted type identifier"))?;
        } else {
            while let Some(c) = self.text[self.at..].chars().next() {
                if c.is_whitespace() || "<>,()\";{}".contains(c) {
                    break;
                }
                self.at += c.len_utf8();
            }
        }
        if start == self.at {
            return Err(self.error("expected a type argument"));
        }
        let mut name = atom(
            &self.text[start..self.at],
            Span {
                start: self.base + start,
                end: self.base + self.at,
            },
        );
        if self.peek() == Some(b'<') {
            if !name.atom().unwrap().starts_with('$') {
                return Err(self.error("type arguments require a $name"));
            }
            let open = self.at;
            self.at += 1;
            self.space()?;
            if self.peek() == Some(b'>') {
                return Err(self.error("expected at least one type argument"));
            }
            let mut items = vec![
                atom(
                    "@inst",
                    Span {
                        start: self.base + open,
                        end: self.base + open,
                    },
                ),
                name,
            ];
            loop {
                items.push(self.read(depth + 1)?);
                self.space()?;
                match self.peek() {
                    Some(b'>') => {
                        self.at += 1;
                        break;
                    }
                    Some(b',') => {
                        self.at += 1;
                        self.space()?;
                        if self.peek() == Some(b'>') {
                            self.at += 1;
                            break;
                        }
                    }
                    None => {
                        return Err(self.error("unclosed type argument list; expected ',' or '>'"));
                    }
                    _ => return Err(self.error("expected ',' or '>' after type argument")),
                }
            }
            let text = &self.text[start..self.at];
            name = if self.literal_types.contains(text) {
                atom(
                    text,
                    Span {
                        start: self.base + start,
                        end: self.base + self.at,
                    },
                )
            } else {
                Sexp {
                    kind: Kind::List(items),
                    span: Span {
                        start: self.base + start,
                        end: self.base + self.at,
                    },
                    context: SyntaxContext::default(),
                }
            };
        }
        Ok(name)
    }
}

pub(crate) fn parse(
    source: &str,
    node: &Sexp,
    literal_types: &BTreeSet<String>,
) -> Result<Sexp, Error> {
    let text = node.atom().unwrap();
    let mut parser = Parser {
        source,
        text,
        base: node.span.start,
        at: 0,
        literal_types,
    };
    let out = parser.read(0)?;
    parser.space()?;
    if parser.at != text.len() {
        return Err(parser.error("unexpected text after type arguments"));
    }
    Ok(out)
}
