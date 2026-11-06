//! Hygienic, bounded expansion into the existing checked ownership IR.
use crate::{
    Error,
    ir::Span,
    sexpr::{Kind, Sexp},
};
use std::collections::BTreeSet;

struct Expander {
    used: BTreeSet<String>,
    next: usize,
}
impl Expander {
    fn fresh(&mut self) -> String {
        loop {
            let name = format!("$scoped_{}", self.next);
            self.next += 1;
            if self.used.insert(name.clone()) {
                return name;
            }
        }
    }
    fn template(&self, text: &str, span: Span, args: &[(&str, &Sexp)]) -> Result<Sexp, Error> {
        fn replace(n: &mut Sexp, span: Span, args: &[(&str, &Sexp)]) {
            if let Some((_, value)) = args.iter().find(|(key, _)| n.atom() == Some(*key)) {
                *n = (*value).clone();
                return;
            }
            n.span = span;
            if let Kind::List(items) = &mut n.kind {
                for child in items {
                    replace(child, span, args);
                }
            }
        }
        let mut node = crate::sexpr::parse(text)?;
        replace(&mut node, span, args);
        Ok(node)
    }
    fn walk(&mut self, source: &str, n: &mut Sexp) -> Result<(), Error> {
        let head = n.head().unwrap_or("").to_owned();
        let Kind::List(items) = &mut n.kind else {
            return Ok(());
        };
        for child in items.iter_mut().skip(1) {
            self.walk(source, child)?;
        }
        if head == "result.do" {
            if items.len() < 3 {
                return Err(Error::at(
                    source,
                    n.span,
                    "expected (%result.do ($name result-expr)... final-result); use (() result-expr) for unit",
                ));
            }
            let mut body = items.last().unwrap().clone();
            for binding in items[1..items.len() - 1].iter().rev() {
                let Some([name, subject]) = binding.list() else {
                    return Err(Error::at(
                        source,
                        binding.span,
                        "expected ($name result-expr) or (() result-expr)",
                    ));
                };
                let names = if name.list().is_some_and(|names| names.is_empty()) {
                    name.clone()
                } else if name.atom().is_some_and(|name| name.starts_with('$')) {
                    Sexp {
                        kind: Kind::List(vec![name.clone()]),
                        span: name.span,
                    }
                } else {
                    return Err(Error::at(
                        source,
                        name.span,
                        "result.do binding must be $name or ()",
                    ));
                };
                body = self.template(
                    "(result.let @names @subject @body)",
                    n.span,
                    &[("@names", &names), ("@subject", subject), ("@body", &body)],
                )?;
                // Check incrementally so a flat source cannot construct an
                // unbounded nested tree before the final expansion limit.
                limit(source, &body)?;
            }
            *n = body;
        } else if head == "result.let" {
            // The checker supplies omitted types from its local context.
            if items.len() == 4 {
                return Ok(());
            }
            if items.len() != 6
                || items[2].head() != Some("result")
                || items[2].list().unwrap().len() != 3
            {
                return Err(Error::at(
                    source,
                    n.span,
                    "expected (result.let ($value) (result T E) U subject body)",
                ));
            }
            let result = items[2].list().unwrap();
            let err = self.fresh();
            let (bind, value) = if result[2].atom() == Some("unit") {
                (String::new(), String::new())
            } else {
                (err.clone(), format!("(local.get {err})"))
            };
            *n = self.template(&format!("(match (result @t @e) (result (result @u @e)) @subject (case ok @binders @body) (case err ({bind}) (result.err @u @e {value})))"),n.span,
                &[("@t",&result[1]),("@e",&result[2]),("@u",&items[3]),("@subject",&items[4]),("@binders",&items[1]),("@body",&items[5])])?;
        } else if head == "stream.fold-result" {
            if items.len() == 5 {
                return Ok(());
            }
            if items.len() != 8
                || items[6].list().is_none_or(|b| {
                    b.len()
                        != if items[1].atom() == Some("unit") {
                            1
                        } else {
                            2
                        }
                })
            {
                return Err(Error::at(
                    source,
                    n.span,
                    "expected (stream.fold-result T S E source initial ($state $item) body)",
                ));
            }
            let binders = items[6].list().unwrap();
            if binders.len() == 2 && binders[0].atom() == binders[1].atom() {
                return Err(Error::at(
                    source,
                    n.span,
                    "fold state and item bindings must differ",
                ));
            }
            let label = self.fresh();
            let lines = self.fresh();
            let next = self.fresh();
            let item = self.fresh();
            let line = self.fresh();
            let code = self.fresh();
            let state = self.fresh();
            let (error_binding, error_value) = if items[3].atom() == Some("unit") {
                (String::new(), String::new())
            } else {
                (code.clone(), format!("(local.get {code})"))
            };
            let payload_binding = if items[1].atom() == Some("unit") {
                "()"
            } else {
                "(@item)"
            };
            let text = format!(
                r#"(loop {label}
              (bind {lines} (stream (result @t @e)) @source) (bind @state @s @initial) (result (result @s @e))
              (match (pair (stream (result @t @e)) (option (result @t @e))) (result (result @s @e)) (stream.next (result @t @e) (move {lines}))
                (case pair ({next} {item})
                  (match (option (result @t @e)) (result (result @s @e)) (local.get {item})
                    (case none () (block (result (result @s @e)) (stream.close (result @t @e) (move {next})) (result.ok @s @e (local.get @state))))
                    (case some ({line})
                      (match (result @t @e) (result (result @s @e)) (local.get {line})
                        (case err ({error_binding}) (block (result (result @s @e)) (stream.close (result @t @e) (move {next})) (result.err @s @e {error_value})))
                        (case ok {payload_binding}
                          (match (result @s @e) (result (result @s @e)) @body
                            (case err ({error_binding}) (block (result (result @s @e)) (stream.close (result @t @e) (move {next})) (result.err @s @e {error_value})))
                            (case ok ({state}) (recur {label} (move {next}) (local.get {state})))))))))))"#
            );
            *n = self.template(
                &text,
                n.span,
                &[
                    ("@t", &items[1]),
                    ("@s", &items[2]),
                    ("@e", &items[3]),
                    ("@source", &items[4]),
                    ("@initial", &items[5]),
                    ("@state", &binders[0]),
                    ("@item", binders.get(1).unwrap_or(&binders[0])),
                    ("@body", &items[7]),
                ],
            )?;
        }
        if matches!(head.as_str(), "result.let" | "stream.fold-result") {
            limit(source, n)?;
        }
        Ok(())
    }
}
pub(crate) fn expand(source: &str, root: &Sexp) -> Result<Sexp, Error> {
    let mut used = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(n) = pending.pop() {
        if let Some(a) = n.atom() {
            used.insert(a.to_owned());
        }
        if let Some(items) = n.list() {
            pending.extend(items);
        }
    }
    let mut output = root.clone();
    Expander { used, next: 0 }.walk(source, &mut output)?;
    limit(source, &output)?;
    Ok(output)
}
fn limit(source: &str, output: &Sexp) -> Result<(), Error> {
    let mut pending = vec![(output, 0)];
    let mut nodes = 0;
    while let Some((n, depth)) = pending.pop() {
        nodes += 1;
        if depth > 256 || nodes > 1_000_000 {
            return Err(Error::at(source, n.span, "macro expansion limit exceeded"));
        }
        if let Some(items) = n.list() {
            pending.extend(items.iter().map(|n| (n, depth + 1)));
        }
    }
    Ok(())
}
