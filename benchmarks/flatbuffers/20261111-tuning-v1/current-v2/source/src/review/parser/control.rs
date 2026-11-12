use super::*;

impl Parser<'_> {
    fn type_parameters(&mut self) -> Result<Option<Vec<Sexp>>, Error> {
        if self.peek() != "<" {
            return Ok(None);
        }
        self.expect("<")?;
        let mut types = Vec::new();
        while !self.peek().starts_with('>') {
            types.push(self.ty()?);
            if !self.peek().starts_with('>') {
                self.expect(",")?;
            }
        }
        self.expect(">")?;
        Ok(Some(types))
    }

    fn condition(&mut self) -> Result<Sexp, Error> {
        let open = self.span();
        self.expect("(")?;
        self.parenthesized(open)
    }

    // Braces delimit a native then/else body; they do not add a block or return.
    fn branch(&mut self, head: &str) -> Result<Sexp, Error> {
        let start = self.span();
        self.expect("{")?;
        let body = if self.peek() == "}" {
            vec![]
        } else {
            vec![self.expr()?]
        };
        self.expect("}")?;
        let span = Span {
            start: start.start,
            end: self.tokens[self.at - 1].span.end,
        };
        Ok(list(head, body, span))
    }

    pub(super) fn conditional(&mut self, start: Span) -> Result<Sexp, Error> {
        let mut fields = Vec::new();
        if let Some(types) = self.type_parameters()? {
            fields.push(list("result", types, start));
        }
        fields.push(self.condition()?);
        fields.push(self.branch("then")?);
        if self.peek() == "else" {
            let mark = self.take()?.span;
            fields.push(if self.peek() == "if" {
                let body = self.expr()?;
                list("else", vec![body], mark)
            } else {
                self.branch("else")?
            });
        }
        let span = Span {
            start: start.start,
            end: self.tokens[self.at - 1].span.end,
        };
        Ok(located_form("if", fields, span, start))
    }

    pub(super) fn matching(&mut self, start: Span) -> Result<Sexp, Error> {
        let mut fields = Vec::new();
        if let Some(mut types) = self.type_parameters()? {
            if types.is_empty() || types.len() > 2 {
                return Err(fail(
                    self.source,
                    start,
                    "match requires a variant type and at most one result type",
                ));
            }
            fields.push(types.remove(0));
            if let Some(result) = types.pop() {
                let span = result.span;
                fields.push(list("result", vec![result], span));
            }
        }
        fields.push(self.condition()?);
        self.expect("{")?;
        let mut arms = 0;
        while self.peek() != "}" {
            let arm_start = self.span();
            let keyword = self.take()?;
            let (head, mut args) = match keyword.text.as_str() {
                "case" => {
                    let label = self.take()?;
                    if !variant_name(&label.text) {
                        return Err(fail(
                            self.source,
                            label.span,
                            "expected an ASCII variant identifier",
                        ));
                    }
                    let open = self.span();
                    self.expect("(")?;
                    let names = self.sequence(")")?;
                    let span = Span {
                        start: open.start,
                        end: self.tokens[self.at - 1].span.end,
                    };
                    (
                        "@case",
                        vec![
                            atom(label.text, label.span),
                            Sexp {
                                kind: Kind::List(names),
                                span,
                                context: Default::default(),
                            },
                        ],
                    )
                }
                "default" => ("@default", vec![]),
                _ => {
                    return Err(fail(self.source, keyword.span, "expected case or default"));
                }
            };
            self.expect("=>")?;
            args.push(self.expr()?);
            let span = Span {
                start: arm_start.start,
                end: self.tokens[self.at - 1].span.end,
            };
            let head_span = Span {
                start: arm_start.start,
                end: keyword.span.end,
            };
            fields.push(located_form(head, args, span, head_span));
            arms += 1;
            if self.peek() != "}" {
                self.expect(",")?;
            }
        }
        self.expect("}")?;
        if arms == 0 {
            return Err(fail(self.source, start, "match needs at least one clause"));
        }
        let span = Span {
            start: start.start,
            end: self.tokens[self.at - 1].span.end,
        };
        Ok(located_form("@match", fields, span, start))
    }
}
