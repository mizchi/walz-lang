//! Resume elaboration at an expression boundary, keeping its checked parents.
use super::*;

impl Checker<'_, '_> {
    pub(super) fn expr_expected(
        &mut self,
        node: &Sexp,
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
        expected: Option<&Type>,
    ) -> Result<Expr, Error> {
        if self.tables.macros.is_none_or(|m| m.definitions.is_empty()) {
            return self.expr_expected_once(node, scope, loops, tail, expected);
        }
        if self.expression_depth >= 256 {
            return Err(self
                .tables
                .error(node, "expression elaboration nesting limit exceeded")
                .with_code("E_MACRO_LIMIT"));
        }
        self.expression_depth += 1;
        // Only macros and expressions with speculative type queries need a
        // retry frame. Ordinary ancestors keep the original small call frame.
        let boundary = node.head().is_some_and(|h| h.starts_with("@$"))
            || node.head() == Some("stream.fold-result")
            || (node.head() == Some("call")
                && node
                    .list()
                    .and_then(|p| p.get(1))
                    .and_then(Sexp::atom)
                    .is_some_and(|name| {
                        self.tables
                            .inference
                            .is_some_and(|i| i.call(name).is_some())
                    }));
        let result = if boundary {
            self.elaborate_expression(node, scope, loops, tail, expected)
        } else {
            self.expr_expected_once(node, scope, loops, tail, expected)
        };
        self.expression_depth -= 1;
        result
    }

    fn elaborate_expression(
        &mut self,
        node: &Sexp,
        scope: &Scope,
        loops: &[LoopContext],
        tail: bool,
        expected: Option<&Type>,
    ) -> Result<Expr, Error> {
        let environment = self.tables.macros.expect("checker macro environment");
        let mut syntax = environment
            .replacement(node)
            .map(Cow::Owned)
            .unwrap_or(Cow::Borrowed(node));
        if syntax.head().is_some_and(|h| h.starts_with("@$")) {
            // A previous speculative operator check may have expanded an input.
            // Replay its syntax, never its temporary LocalIds or LoopIds.
            environment.apply_expansions(syntax.to_mut());
        }
        loop {
            let locals = self.locals.len();
            let next_loop = self.next_loop;
            let checked = self.expr_expected_once(&syntax, scope, loops, tail, expected);
            if self.probing
                || !checked
                    .as_ref()
                    .is_err_and(|e| e.diagnostic().code == "E_MACRO_PENDING")
            {
                return checked;
            }
            self.locals.truncate(locals);
            self.next_loop = next_loop;
            // The immutable type/function/heap environment still applies. A new
            // specialization returns E_MACRO_PENDING to the module restart path.
            environment.inline(
                self.tables.source,
                syntax.to_mut(),
                self.tables.aliases.expect("checker aliases"),
                self.tables.inference.expect("checker inference"),
                self.expression_depth - 1,
            )?;
        }
    }
}
