use crate::Error;
use crate::ir::Span;

#[derive(Clone, Debug)]
pub(crate) struct Sexp {
    pub kind: Kind,
    pub span: Span,
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
    let mut reader = Reader {
        source,
        position: 0,
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
            self.space()?;
            if items.len() == 1 {
                let prefix = items[0]
                    .atom()
                    .and_then(|s| s.strip_prefix('%'))
                    .and_then(|s| s.strip_suffix("::"));
                if let Some(name) = prefix.filter(|_| self.rest().starts_with('{')) {
                    if !name.starts_with('$') || name.len() == 1 {
                        return Err(self
                            .error("compact construction requires a $type identifier; use %$Type::{...}")
                            .with_code("E_STRUCT_LITERAL"));
                    }
                    let ty = Sexp {
                        kind: Kind::Atom(name.to_owned()),
                        span: items[0].span,
                    };
                    items[0].kind = Kind::Atom("%struct.new".into());
                    items.push(ty);
                }
            }
            if self.rest().starts_with('{') {
                if items.len() != 2 || items[0].atom() != Some("%struct.new") {
                    return Err(self
                        .error("field maps belong to %struct.new or %$Type::")
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
                return Err(Error::at(
                    self.source,
                    Span { start, end: start },
                    "unclosed '('; expected ')'",
                ));
            }
            items.push(self.read(depth + 1)?);
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
            kind,
            span: Span {
                start,
                end: self.position,
            },
        })
    }
}
