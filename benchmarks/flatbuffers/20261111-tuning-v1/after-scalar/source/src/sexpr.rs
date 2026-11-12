use crate::Error;
use crate::ir::Span;

#[cfg(test)]
thread_local! {
    static PARSE_LOG: std::cell::RefCell<Vec<(usize, u64)>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub(crate) fn take_parse_log() -> Vec<(usize, u64)> {
    PARSE_LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

#[cfg(test)]
pub(crate) fn parse_key(source: &str) -> (usize, u64) {
    use std::hash::Hasher;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    hash.write(source.as_bytes());
    (source.len(), hash.finish())
}

#[derive(Clone, Debug)]
pub(crate) struct Sexp {
    pub kind: Kind,
    pub span: Span,
    pub context: SyntaxContext,
}

/// Identity is snapshot-local. Expansion context survives syntax cloning and
/// lowering; source byte ranges alone cannot distinguish duplicated arguments.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SyntaxContext {
    pub id: usize,
    pub expansion: Option<usize>,
    pub template: Option<Span>,
    pub global: bool,
}

pub(crate) fn identify(root: &mut Sexp) {
    fn walk(node: &mut Sexp, next: &mut usize) {
        node.context.id = *next;
        *next += 1;
        if let Kind::List(items) = &mut node.kind {
            for child in items {
                walk(child, next);
            }
        }
    }
    walk(root, &mut 1);
}

#[derive(Clone, Debug)]
pub(crate) enum Kind {
    Atom(String),
    Text(String),
    Bytes(Vec<u8>),
    List(Vec<Sexp>),
}

impl Sexp {
    pub fn atom(&self) -> Option<&str> {
        if let Kind::Atom(s) = &self.kind {
            Some(s)
        } else {
            None
        }
    }
    pub fn text(&self) -> Option<&str> {
        match &self.kind {
            Kind::Text(s) => Some(s),
            Kind::Bytes(bytes) => std::str::from_utf8(bytes).ok(),
            _ => None,
        }
    }
    pub fn list(&self) -> Option<&[Sexp]> {
        if let Kind::List(s) = &self.kind {
            Some(s)
        } else {
            None
        }
    }
    pub fn head(&self) -> Option<&str> {
        self.list()?.first()?.atom()
    }
}

pub(crate) fn parse(source: &str) -> Result<Sexp, Error> {
    #[cfg(test)]
    PARSE_LOG.with(|log| log.borrow_mut().push(parse_key(source)));
    let mut reader = Reader {
        source,
        position: 0,
        recover: false,
    };
    reader.space()?;
    let expression = reader.read(0)?;
    reader.space()?;
    if reader.position != source.len() {
        return Err(reader.error("expected one module"));
    }
    Ok(expression)
}

struct Reader<'a> {
    source: &'a str,
    position: usize,
    recover: bool,
}

/// Editor-only prefix syntax. The returned tree is never passed to checking or
/// code generation. Strict parsing remains the acceptance boundary.
#[cfg(not(target_family = "wasm"))]
pub(crate) fn editor_prefix(source: &str) -> Result<Sexp, Error> {
    let mut reader = Reader {
        source,
        position: 0,
        recover: true,
    };
    reader.space()?;
    let syntax = reader.read(0)?;
    if syntax.head() != Some("module") {
        return Err(reader.error("expected module"));
    }
    Ok(syntax)
}

impl Reader<'_> {
    fn fields(&mut self, depth: usize) -> Result<Vec<Sexp>, Error> {
        let start = self.position;
        self.position += 1;
        let mut fields = Vec::new();
        loop {
            self.space()?;
            if self.rest().starts_with('}') {
                self.position += 1;
                return Ok(fields);
            }
            if self.rest().is_empty() || self.rest().starts_with(')') {
                return Err(Error::at(
                    self.source,
                    Span {
                        start,
                        end: start + 1,
                    },
                    "unclosed field map; expected '}'",
                )
                .with_code("E_STRUCT_LITERAL"));
            }
            let mut key = self.read(depth + 1)?;
            let name = key
                .atom()
                .and_then(|s| s.strip_prefix(':'))
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    Error::at(self.source, key.span, "expected :field in field map")
                        .with_code("E_STRUCT_LITERAL")
                })?;
            key.kind = Kind::Atom(format!("${name}"));
            self.space()?;
            if self.rest().starts_with('}')
                || self.rest().starts_with(')')
                || self.rest().is_empty()
            {
                return Err(
                    Error::at(self.source, key.span, "field needs an initializer")
                        .with_code("E_STRUCT_LITERAL"),
                );
            }
            let value = self.read(depth + 1)?;
            fields.push(Sexp {
                context: SyntaxContext::default(),
                span: Span {
                    start: key.span.start,
                    end: value.span.end,
                },
                kind: Kind::List(vec![key, value]),
            });
        }
    }

    fn error(&self, message: &str) -> Error {
        Error::at(
            self.source,
            Span {
                start: self.position,
                end: self.position,
            },
            message,
        )
    }

    fn rest(&self) -> &str {
        &self.source[self.position..]
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.rest().chars().next()?;
        self.position += ch.len_utf8();
        Some(ch)
    }

    fn space(&mut self) -> Result<(), Error> {
        loop {
            if self.rest().starts_with(";;") {
                while self.advance().is_some_and(|ch| ch != '\n') {}
            } else if self.rest().starts_with("(;") {
                self.position += 2;
                let mut depth = 1;
                while depth > 0 {
                    if self.rest().starts_with("(;") {
                        depth += 1;
                        self.position += 2;
                    } else if self.rest().starts_with(";)") {
                        depth -= 1;
                        self.position += 2;
                    } else if self.advance().is_none() {
                        return Err(self.error("unterminated comment"));
                    }
                }
            } else if self.rest().chars().next().is_some_and(char::is_whitespace) {
                self.advance();
            } else {
                return Ok(());
            }
        }
    }

    fn list(&mut self, start: usize, depth: usize) -> Result<Vec<Sexp>, Error> {
        let mut items: Vec<Sexp> = Vec::new();
        loop {
            if let Err(error) = self.space() {
                if !self.recover {
                    return Err(error);
                }
                self.position = self.source.len();
                break;
            }
            if items.len() == 1 {
                let prefix = items[0]
                    .atom()
                    .and_then(|s| s.strip_prefix('@'))
                    .and_then(|s| s.strip_suffix("::"));
                if let Some(name) = prefix.filter(|_| self.rest().starts_with('{')) {
                    if !name.starts_with('$') || name.len() == 1 {
                        return Err(self
                            .error("compact construction requires a $type identifier; use @$Type::{...}")
                            .with_code("E_STRUCT_LITERAL"));
                    }
                    let ty = Sexp {
                        context: SyntaxContext::default(),
                        kind: Kind::Atom(name.to_owned()),
                        span: items[0].span,
                    };
                    items[0].kind = Kind::Atom("@struct.new".into());
                    items.push(ty);
                }
            }
            if self.rest().starts_with('{') {
                if items.len() == 1 && matches!(items[0].atom(), Some("@let" | "@for-each")) {
                    let start = self.position;
                    let mut fields = vec![Sexp {
                        context: SyntaxContext::default(),
                        kind: Kind::Atom("@fields".into()),
                        span: Span { start, end: start },
                    }];
                    fields.extend(self.fields(depth + 1)?);
                    items.push(Sexp {
                        context: SyntaxContext::default(),
                        kind: Kind::List(fields),
                        span: Span {
                            start,
                            end: self.position,
                        },
                    });
                    continue;
                }
                if items.len() != 2 || items[0].atom() != Some("@struct.new") {
                    return Err(self
                        .error("field maps belong to @struct.new, @$Type::, or a @let / @for-each pattern")
                        .with_code("E_STRUCT_LITERAL"));
                }
                items.extend(self.fields(depth + 1)?);
                self.space()?;
                if !self.rest().starts_with(')') {
                    return Err(self
                        .error("expected ')' after field map")
                        .with_code("E_STRUCT_LITERAL"));
                }
                self.position += 1;
                break;
            }
            if self.rest().starts_with(')') {
                self.position += 1;
                break;
            }
            if self.rest().is_empty() {
                if self.recover {
                    break;
                }
                return Err(Error::at(
                    self.source,
                    Span { start, end: start },
                    "unclosed '('; expected ')'",
                ));
            }
            match self.read(depth + 1) {
                Ok(item) => items.push(item),
                Err(_) if self.recover => {
                    self.position = self.source.len();
                    break;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(items)
    }

    fn read(&mut self, depth: usize) -> Result<Sexp, Error> {
        if depth > 256 {
            return Err(self.error("S-expression nesting limit exceeded"));
        }
        self.space()?;
        let start = self.position;
        let kind =
            match self.advance() {
                Some('(') => Kind::List(self.list(start, depth)?),
                Some(')') => return Err(self.error("unexpected ')'")),
                Some('{' | '}') => {
                    return Err(self
                        .error("unexpected field map delimiter")
                        .with_code("E_STRUCT_LITERAL"));
                }
                Some('"') => {
                    let mut offset = start;
                    let token = wast::lexer::Lexer::new(self.source)
                        .parse(&mut offset)
                        .map_err(|error| self.error(&error.to_string()))?
                        .ok_or_else(|| self.error("unterminated string"))?;
                    self.position = offset;
                    if token.kind != wast::lexer::TokenKind::String {
                        return Err(Error::at(self.source, Span { start, end: offset },
                        "expected a separate string literal; add whitespace before the next token")
                        .with_code("E_LEX_TOKEN"));
                    }
                    let bytes = token.string(self.source).into_owned();
                    match String::from_utf8(bytes) {
                        Ok(text) => Kind::Text(text),
                        Err(error) => Kind::Bytes(error.into_bytes()),
                    }
                }
                Some(_) => {
                    let mut offset = start;
                    wast::lexer::Lexer::new(self.source)
                        .parse(&mut offset)
                        .map_err(|error| self.error(&error.to_string()))?
                        .ok_or_else(|| self.error("expected token"))?;
                    self.position = offset;
                    Kind::Atom(self.source[start..self.position].to_owned())
                }
                None => return Err(self.error("expected module")),
            };
        Ok(Sexp {
            context: SyntaxContext::default(),
            kind,
            span: Span {
                start,
                end: self.position,
            },
        })
    }
}

/// Editor-only delimiter completion. Tokens are never invented and strings /
/// nested comments are lexed by WAT's lexer. The result must not be compiled.
#[cfg(not(target_family = "wasm"))]
pub(crate) fn editor_closed(source: &str) -> Option<String> {
    let lexer = wast::lexer::Lexer::new(source);
    let mut offset = 0;
    let mut depth = 0usize;
    while let Some(token) = lexer.parse(&mut offset).ok()? {
        match token.kind {
            wast::lexer::TokenKind::LParen => {
                depth += 1;
                if depth > 256 {
                    return None;
                }
            }
            wast::lexer::TokenKind::RParen => {
                depth = depth.checked_sub(1)?;
            }
            _ => {}
        }
    }
    if depth == 0 {
        return None;
    }
    let completed = format!("{source}\n{}", ")".repeat(depth));
    parse(&completed).ok()?;
    Some(completed)
}

#[cfg(test)]
mod editor_completion_tests {
    #[test]
    fn only_missing_parentheses_are_completed_and_lexical_contents_are_preserved() {
        let source = "(module (; nested (; ( ;) ;) (func $f (result i32) (i32.const 42)) ;; )";
        let closed = super::editor_closed(source).unwrap();
        assert!(closed.starts_with(source));
        assert!(super::parse(source).is_err());
        assert!(super::parse(&closed).is_ok());
        for source in [
            "(module)",
            "(module))",
            "(module (func $f \"unterminated",
            "(module (; unclosed",
        ] {
            assert!(super::editor_closed(source).is_none(), "{source}");
        }
    }
}
