use super::*;
mod control;

fn located_form(head: &str, args: Vec<Sexp>, span: Span, head_span: Span) -> Sexp {
    let mut node = list(head, args, span);
    if let Kind::List(items) = &mut node.kind {
        items[0].span = head_span;
    }
    node
}

#[derive(Clone)]
struct Token {
    text: String,
    span: Span,
}
struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    at: usize,
    depth: usize,
    comments: Vec<String>,
}
impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Result<Self, Error> {
        let mut tokens = Vec::new();
        let mut comments = Vec::new();
        let mut at = 0;
        while at < source.len() {
            let rest = &source[at..];
            let c = rest.chars().next().unwrap();
            if c.is_whitespace() {
                at += c.len_utf8();
                continue;
            }
            if rest.starts_with("//") {
                let length = rest.find('\n').unwrap_or(rest.len());
                let note = rest[2..length].trim_start();
                if !note.starts_with("Walz review syntax:") {
                    comments.push(note.into());
                }
                at += length;
                continue;
            }
            if rest.starts_with("/*") {
                let end = rest.find("*/").ok_or_else(|| {
                    fail(
                        source,
                        Span {
                            start: at,
                            end: at + 2,
                        },
                        "unclosed review comment",
                    )
                })?;
                comments.extend(rest[2..end].lines().map(str::to_owned));
                at += end + 2;
                continue;
            }
            let start = at;
            if c == '"' {
                at += 1;
                let mut escaped = false;
                while at < source.len() {
                    let c = source[at..].chars().next().unwrap();
                    at += c.len_utf8();
                    if !escaped && c == '"' {
                        break;
                    }
                    escaped = !escaped && c == '\\';
                }
                serde_json::from_str::<String>(&source[start..at]).map_err(|_| {
                    fail(
                        source,
                        Span { start, end: at },
                        "expected a JSON string literal",
                    )
                })?;
            } else if let Some(op) = [
                ">>>", "===", "!==", "=>", "&&", "||", "<<", ">>", "<=", ">=", "==", "!=", "+",
                "-", "*", "/", "!", "&", "|", "^", "<", ">", "=",
            ]
            .iter()
            .find(|op| {
                rest.starts_with(**op)
                    && !(**op == "-" && rest.as_bytes().get(1).is_some_and(u8::is_ascii_digit))
            }) {
                at += op.len();
            } else if "(){}[],:;@".contains(c) {
                at += 1;
            } else {
                while at < source.len() {
                    let c = source[at..].chars().next().unwrap();
                    if c.is_whitespace() || "(){}[],:;=<>@\"+*/!&|^".contains(c) {
                        break;
                    }
                    at += c.len_utf8();
                }
            }
            tokens.push(Token {
                text: source[start..at].into(),
                span: Span { start, end: at },
            });
            if tokens.len() > 1_000_000 {
                return Err(fail(
                    source,
                    Span { start, end: at },
                    "review token limit exceeded",
                ));
            }
        }
        let mut nesting = 0usize;
        for token in &tokens {
            if matches!(token.text.as_str(), "(" | "[" | "{") {
                nesting += 1;
                if nesting > 256 {
                    return Err(fail(source, token.span, "review nesting limit exceeded"));
                }
            } else if matches!(token.text.as_str(), ")" | "]" | "}") {
                nesting = nesting.saturating_sub(1);
            }
        }
        Ok(Self {
            source,
            tokens,
            at: 0,
            depth: 0,
            comments,
        })
    }
    fn peek(&self) -> &str {
        self.tokens
            .get(self.at)
            .map(|t| t.text.as_str())
            .unwrap_or("")
    }
    fn span(&self) -> Span {
        self.tokens.get(self.at).map(|t| t.span).unwrap_or(Span {
            start: self.source.len(),
            end: self.source.len(),
        })
    }
    fn take(&mut self) -> Result<Token, Error> {
        let token = self
            .tokens
            .get(self.at)
            .cloned()
            .ok_or_else(|| fail(self.source, self.span(), "unexpected end of review source"))?;
        self.at += 1;
        Ok(token)
    }
    fn expect(&mut self, text: &str) -> Result<(), Error> {
        if text == ">" && self.peek().starts_with('>') && self.peek().len() > 1 {
            self.tokens[self.at].text.remove(0);
            self.tokens[self.at].span.start += 1;
            return Ok(());
        }
        if self.peek() != text {
            return Err(fail(
                self.source,
                self.span(),
                &format!("expected `{text}`"),
            ));
        }
        self.at += 1;
        Ok(())
    }
    fn sequence(&mut self, end: &str) -> Result<Vec<Sexp>, Error> {
        let mut items = Vec::new();
        while self.peek() != end {
            items.push(self.expr()?);
            if self.peek() != end {
                self.expect(",")?;
            }
        }
        self.expect(end)?;
        Ok(items)
    }
    fn expr(&mut self) -> Result<Sexp, Error> {
        if self.depth >= 256 {
            return Err(fail(
                self.source,
                self.span(),
                "review nesting limit exceeded",
            ));
        }
        self.depth += 1;
        let result = self.expression();
        self.depth -= 1;
        result
    }
    fn expression(&mut self) -> Result<Sexp, Error> {
        let token = self.take()?;
        if matches!(self.peek(), "(" | "<") {
            if token.text == "if" {
                return self.conditional(token.span);
            }
            if token.text == "match" {
                return self.matching(token.span);
            }
        }
        if token.text == "(" {
            return self.parenthesized(token.span);
        }
        if token.text == "[" {
            let items = self.sequence("]")?;
            return Ok(Sexp {
                kind: Kind::List(items),
                span: token.span,
                context: Default::default(),
            });
        }
        if token.text.starts_with('"') {
            return Ok(Sexp {
                kind: Kind::Text(serde_json::from_str(&token.text).unwrap()),
                span: token.span,
                context: Default::default(),
            });
        }
        if !identifier(&token.text) && !number(&token.text) {
            return Err(fail(
                self.source,
                token.span,
                "expected a symbol, literal, or structural call",
            ));
        }
        let name = token
            .text
            .strip_prefix("walz.")
            .map(|s| format!("@{s}"))
            .or_else(|| token.text.strip_prefix("wat.").map(str::to_owned))
            .unwrap_or(token.text.clone());
        if self.peek() != "(" {
            return Ok(atom(name, token.span));
        }
        self.expect("(")?;
        let mut args = self.sequence(")")?;
        let span = Span {
            start: token.span.start,
            end: self.tokens[self.at - 1].span.end,
        };
        if token.text == "sexpr.atom" || token.text == "sexpr.bytes" {
            if args.len() != 1 {
                return Err(fail(
                    self.source,
                    token.span,
                    "expected one encoded literal",
                ));
            }
            let value = args[0]
                .text()
                .ok_or_else(|| fail(self.source, args[0].span, "expected a string literal"))?;
            if token.text == "sexpr.atom" {
                if !valid_atom(value) {
                    return Err(fail(
                        self.source,
                        args[0].span,
                        "encoded atom must be one WAT symbol",
                    ));
                }
                return Ok(atom(value, span));
            }
            if value.len() % 2 != 0 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(fail(
                    self.source,
                    args[0].span,
                    "expected hexadecimal bytes",
                ));
            }
            let bytes = (0..value.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap())
                .collect();
            return Ok(Sexp {
                kind: Kind::Bytes(bytes),
                span: token.span,
                context: Default::default(),
            });
        }
        if token.text == "sexpr.form" {
            if args.is_empty() {
                return Err(fail(self.source, token.span, "expected a form name"));
            }
            let head = args.remove(0);
            let name = head
                .text()
                .ok_or_else(|| fail(self.source, head.span, "expected a quoted form name"))?;
            if !valid_atom(name) {
                return Err(fail(
                    self.source,
                    head.span,
                    "form name must be one WAT symbol",
                ));
            }
            return Ok(located_form(name, args, span, head.span));
        }
        if token.text.starts_with('$') {
            args.insert(0, atom(name, token.span));
            return Ok(list("call", args, span));
        }
        Ok(located_form(&name, args, span, token.span))
    }

    fn parenthesized(&mut self, open: Span) -> Result<Sexp, Error> {
        let left = self.expr()?;
        if self.peek() == ")" {
            self.expect(")")?;
            return Ok(left);
        }
        let op = self.take()?;
        let name = match op.text.as_str() {
            "===" => "=",
            "!==" => "!=",
            name if binary(name).is_some() => name,
            _ => {
                return Err(fail(
                    self.source,
                    op.span,
                    "expected a Walz binary operator",
                ));
            }
        };
        let right = self.expr()?;
        self.expect(")")?;
        let span = Span {
            start: open.start,
            end: self.tokens[self.at - 1].span.end,
        };
        Ok(located_form(name, vec![left, right], span, op.span))
    }
    fn ty(&mut self) -> Result<Sexp, Error> {
        if self.depth >= 256 {
            return Err(fail(
                self.source,
                self.span(),
                "review type nesting limit exceeded",
            ));
        }
        self.depth += 1;
        let result = self.type_inner();
        self.depth -= 1;
        result
    }
    fn type_inner(&mut self) -> Result<Sexp, Error> {
        let node = self.expr()?;
        if self.peek() != "<" {
            return Ok(node);
        }
        self.expect("<")?;
        let mut args = Vec::new();
        while !self.peek().starts_with('>') {
            args.push(self.ty()?);
            if !self.peek().starts_with('>') {
                self.expect(",")?;
            }
        }
        self.expect(">")?;
        Ok(match node.atom() {
            Some("ref") if args.len() == 1 => list("ref", args, node.span),
            Some("refNull") if args.len() == 1 => {
                args.insert(0, atom("null", node.span));
                list("ref", args, node.span)
            }
            Some(name) if name.starts_with('$') => {
                args.insert(0, node.clone());
                list("@inst", args, node.span)
            }
            _ => {
                return Err(fail(
                    self.source,
                    node.span,
                    "type arguments require a named Walz type",
                ));
            }
        })
    }
    fn func_body(&mut self) -> Result<Sexp, Error> {
        let mut levels = Vec::<(Span, Vec<Sexp>)>::new();
        loop {
            if self.peek() != "let" {
                break;
            }
            let span = self.take()?.span;
            let mut bindings = Vec::new();
            loop {
                let name = self.take()?;
                if !name.text.starts_with('$') || !simple_name(&name.text) {
                    return Err(fail(
                        self.source,
                        name.span,
                        "let declarations require a simple $name; use walz.let for other names",
                    ));
                }
                let mut fields = vec![atom(name.text, name.span)];
                if self.peek() == ":" {
                    self.expect(":")?;
                    fields.push(list("type", vec![self.ty()?], name.span));
                }
                self.expect("=")?;
                fields.push(self.expr()?);
                bindings.push(Sexp {
                    kind: Kind::List(fields),
                    span: name.span,
                    context: Default::default(),
                });
                if self.peek() != "," {
                    break;
                }
                self.expect(",")?;
            }
            self.expect(";")?;
            levels.push((span, bindings));
            if levels.len() > 256 {
                return Err(fail(
                    self.source,
                    span,
                    "review binding nesting limit exceeded",
                ));
            }
        }
        self.expect("return")?;
        let mut body = self.expr()?;
        self.expect(";")?;
        for (span, mut bindings) in levels.into_iter().rev() {
            if bindings.len() == 1 {
                let Kind::List(mut fields) = bindings.remove(0).kind else {
                    unreachable!()
                };
                fields.push(body);
                body = list("@let", fields, span);
            } else {
                body = list(
                    "@let",
                    vec![
                        Sexp {
                            kind: Kind::List(bindings),
                            span,
                            context: Default::default(),
                        },
                        body,
                    ],
                    span,
                );
            }
        }
        Ok(body)
    }
    fn func(&mut self, decorators: Vec<Sexp>) -> Result<Sexp, Error> {
        let span = self.take()?.span;
        let name = self.take()?;
        if !name.text.starts_with('$') || !simple_name(&name.text) {
            return Err(fail(
                self.source,
                name.span,
                "func declarations require a simple $name; use wat.func for other names",
            ));
        }
        let mut fields = vec![atom(name.text, name.span)];
        fields.extend(
            decorators
                .iter()
                .filter(|d| d.head() == Some("export"))
                .cloned(),
        );
        self.expect("(")?;
        while self.peek() != ")" {
            let name = self.take()?;
            if !name.text.starts_with('$') || !simple_name(&name.text) {
                return Err(fail(
                    self.source,
                    name.span,
                    "parameters require a simple $name",
                ));
            }
            self.expect(":")?;
            let ty = self.ty()?;
            fields.push(list(
                "param",
                vec![atom(name.text, name.span), ty],
                name.span,
            ));
            if self.peek() != ")" {
                self.expect(",")?;
            }
        }
        self.expect(")")?;
        if self.peek() == ":" {
            self.expect(":")?;
            fields.push(list("result", vec![self.ty()?], span));
        }
        fields.extend(
            decorators
                .into_iter()
                .filter(|d| d.head() == Some("@effect")),
        );
        self.expect("{")?;
        fields.push(self.func_body()?);
        self.expect("}")?;
        Ok(list("func", fields, span))
    }
    fn module(&mut self) -> Result<Sexp, Error> {
        let start = self.span();
        let wrapped = self.peek() == "module";
        if wrapped {
            self.expect("module")?;
            self.expect("{")?;
        }
        let mut fields = Vec::new();
        while self.at < self.tokens.len() && !(wrapped && self.peek() == "}") {
            if matches!(self.peek(), "import" | "export") {
                let head = self.take()?;
                self.expect("{")?;
                let mut members = Vec::new();
                while self.peek() != "}" {
                    let kind = if self.peek() == "type" {
                        self.expect("type")?;
                        "type"
                    } else {
                        "func"
                    };
                    let name = self.take()?;
                    if !name.text.starts_with('$') || !identifier(&name.text) {
                        return Err(fail(
                            self.source,
                            name.span,
                            "imports and exports require $names",
                        ));
                    }
                    let mut values = vec![atom(name.text, name.span)];
                    if self.peek() == "as" {
                        self.expect("as")?;
                        let alias = self.take()?;
                        if !alias.text.starts_with('$') || !identifier(&alias.text) {
                            return Err(fail(self.source, alias.span, "aliases require $names"));
                        }
                        values.push(atom(alias.text, alias.span));
                    }
                    members.push(list(kind, values, name.span));
                    if self.peek() != "}" {
                        self.expect(",")?;
                    }
                }
                self.expect("}")?;
                if head.text == "import" {
                    self.expect("from")?;
                    members.insert(0, self.expr()?);
                }
                self.expect(";")?;
                fields.push(list(
                    if head.text == "import" {
                        "@use"
                    } else {
                        "@export"
                    },
                    members,
                    head.span,
                ));
                continue;
            }
            let mut decorators = Vec::new();
            while self.peek() == "@" {
                self.expect("@")?;
                let name = self.take()?;
                self.expect("(")?;
                let args = self.sequence(")")?;
                let head = match name.text.as_str() {
                    "export" => "export",
                    "effect" => "@effect",
                    _ => {
                        return Err(fail(self.source, name.span, "expected @export or @effect"));
                    }
                };
                decorators.push(list(head, args, name.span));
            }
            if self.peek() == "func" {
                fields.push(self.func(decorators)?);
            } else {
                if !decorators.is_empty() {
                    return Err(fail(self.source, self.span(), "annotations require func"));
                }
                fields.push(self.expr()?);
                self.expect(";")?;
            }
        }
        if wrapped {
            self.expect("}")?;
        }
        if self.at != self.tokens.len() {
            return Err(fail(self.source, self.span(), "expected one module"));
        }
        Ok(list("module", fields, start))
    }
}

fn valid_atom(value: &str) -> bool {
    crate::sexpr::parse(&format!("({value})")).is_ok_and(|node| {
        node.list()
            .is_some_and(|p| p.len() == 1 && p[0].atom() == Some(value))
    })
}

pub(super) fn parse(source: &str) -> Result<(Sexp, Vec<String>), Error> {
    let mut parser = Parser::new(source)?;
    let root = parser.module()?;
    Ok((root, parser.comments))
}
