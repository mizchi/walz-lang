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
        if let Kind::Text(s) = &self.kind {
            Some(s)
        } else {
            None
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

    fn read(&mut self, depth: usize) -> Result<Sexp, Error> {
        if depth > 256 {
            return Err(self.error("S-expression nesting limit exceeded"));
        }
        self.space()?;
        let start = self.position;
        let kind =
            match self.advance() {
                Some('(') => {
                    let mut items = Vec::new();
                    loop {
                        self.space()?;
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
                    Kind::List(items)
                }
                Some(')') => return Err(self.error("unexpected ')'")),
                Some('"') => {
                    let mut text = String::new();
                    loop {
                        match self.advance() {
                            Some('"') => break,
                            Some('\\') => text.push(match self.advance() {
                                Some('n') => '\n',
                                Some('r') => '\r',
                                Some('t') => '\t',
                                Some('"') => '"',
                                Some('\\') => '\\',
                                _ => return Err(self.error("unsupported string escape")),
                            }),
                            Some(ch) if !ch.is_control() => text.push(ch),
                            Some(_) => return Err(self.error("control character in string")),
                            None => return Err(self.error("unterminated string")),
                        }
                    }
                    Kind::Text(text)
                }
                Some(_) => {
                    while self.rest().chars().next().is_some_and(|ch| {
                        !ch.is_whitespace() && !matches!(ch, '(' | ')' | '"' | ';')
                    }) {
                        self.advance();
                    }
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
