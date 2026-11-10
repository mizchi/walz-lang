//! Opt-in advice about source operations that constrain GC optimization.
//! These conservative diagnostics are neither cost estimates nor legality checks.
use crate::{
    Error,
    bindings::BindingPolicy,
    diagnostic::{Diagnostic, Label},
    ir::*,
    regions::{MemorySummary, Root},
    source::Source,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub profile: &'static str,
    /// `opaque_wat` means expression-level advice is unavailable, not clean.
    pub coverage: &'static str,
    pub warnings: Vec<Diagnostic>,
}

impl Report {
    fn empty(coverage: &'static str) -> Self {
        Self {
            schema: "walz-performance-v1",
            profile: "binaryen-133-open-world",
            coverage,
            warnings: vec![],
        }
    }

    pub fn located(mut self, source: &Source) -> Self {
        self.warnings = self
            .warnings
            .into_iter()
            .map(|d| source.locate_diagnostic(d))
            .collect();
        self
    }
}

/// Validate all existing contracts before producing optional advice. No host
/// code or optimizer executable is run. Valid native WAT remains accepted.
pub fn check_with_policy(source: &str, policy: &BindingPolicy) -> Result<Report, Error> {
    match crate::check_with_policy(source, policy) {
        Ok(module) => {
            if !module.laws.is_empty() || module.race_free {
                crate::verified::verify(source, &module)?;
            }
            Ok(analyze(source, &module))
        }
        Err(_) => {
            // The standard-WAT / native-macro fallback has the same acceptance
            // boundary as ordinary check, including contracts on extensions.
            let compiled = crate::compile_with_policy(source, policy)?;
            Ok(match compiled.ir {
                Some(module) => analyze(source, &module),
                None => Report::empty("opaque_wat"),
            })
        }
    }
}

fn direct_copy(expression: &Expr) -> bool {
    matches!(
        expression.kind,
        ExprKind::Buffer {
            op: BufferOp::Slice,
            ..
        } | ExprKind::List {
            op: ListOp::Slice,
            ..
        }
    )
}

fn copy_origin(expression: &Expr, copies: &[Option<Span>]) -> Option<Span> {
    if direct_copy(expression) {
        Some(expression.span)
    } else if let ExprKind::Call { function, .. } = expression.kind {
        copies[function.0]
    } else {
        None
    }
}

fn reachable(module: &Module, function: &Function) -> BTreeMap<Span, bool> {
    let mut spans = BTreeMap::new();
    for fact in crate::flow::analyze(module, function) {
        // Generated macro nodes may share a span. Suppress only if all
        // occurrences at that span are known unreachable.
        *spans.entry(fact.span).or_insert(false) |= fact.reachable;
    }
    spans
}

fn find_copy(
    expression: &Expr,
    copies: &[Option<Span>],
    live: &BTreeMap<Span, bool>,
) -> Option<Span> {
    if live.get(&expression.span) == Some(&false) {
        return None;
    }
    copy_origin(expression, copies).or_else(|| {
        expression
            .children()
            .into_iter()
            .find_map(|child| find_copy(child, copies, live))
    })
}

struct Visitor<'a> {
    source: &'a str,
    copies: &'a [Option<Span>],
    live: &'a BTreeMap<Span, bool>,
    host_allocations: BTreeSet<Span>,
    warnings: &'a mut Vec<Diagnostic>,
}

impl Visitor<'_> {
    fn warn(
        &mut self,
        expression: &Expr,
        code: &'static str,
        message: &str,
        help: &str,
    ) -> Option<&mut Diagnostic> {
        let span = expression.span;
        if span.start >= span.end
            || span.end > self.source.len()
            || !self.source.is_char_boundary(span.start)
            || !self.source.is_char_boundary(span.end)
        {
            return None;
        }
        self.warnings.push(Diagnostic {
            code,
            message: message.into(),
            primary: Some(Label::new(self.source, span, "")),
            related: vec![],
            notes: vec!["Conservative advice for Binaryen 133 open-world optimization; this is not a measured hotspot or proof that optimization will fail.".into()],
            help: vec![help.into()],
        });
        self.warnings.last_mut()
    }

    fn visit(&mut self, expression: &Expr, in_loop: bool) {
        if self.live.get(&expression.span) == Some(&false) {
            return;
        }
        if in_loop {
            if let Some(origin) = copy_origin(expression, self.copies) {
                let related = (origin != expression.span
                    && origin.start < origin.end
                    && origin.end <= self.source.len()
                    && self.source.is_char_boundary(origin.start)
                    && self.source.is_char_boundary(origin.end))
                .then(|| {
                    Label::new(
                        self.source,
                        origin,
                        "the called function may copy this range",
                    )
                });
                if let Some(warning) = self.warn(expression, "W_OPT_LOOP_COPY",
                    "range copy in a loop body may allocate and copy data on each iteration",
                    "Consider reading the source with indices or a byte/string view. Move an invariant copy before the loop only when evaluation order, bounds traps, and source retention permit it.") {
                    warning.related.extend(related);
                }
            }
            if self.host_allocations.contains(&expression.span) {
                self.warn(expression, "W_OPT_LOOP_HOST_ESCAPE",
                    "GC allocation in a loop body may escape to a host call, limiting local allocation removal",
                    "Keep the object inside Wasm and pass primitive fields if the host contract permits it, or batch values at the host boundary.");
            }
        }
        if let ExprKind::Loop { bindings, body, .. } = &expression.kind {
            // Initializers run once per entry, not once per back edge. A
            // nested loop's initializers can still repeat in its outer loop.
            for (_, initial) in bindings {
                self.visit(initial, in_loop);
            }
            self.visit(body, true);
        } else {
            for child in expression.children() {
                self.visit(child, in_loop);
            }
        }
    }
}

fn host_allocations(memory: &MemorySummary, function: &Function) -> BTreeSet<Span> {
    memory
        .allocations
        .iter()
        .filter_map(|allocation| match &allocation.site {
            Root::Allocation {
                function: owner,
                span,
                ..
            } if owner == &function.name && allocation.escapes.host => Some(*span),
            _ => None,
        })
        .collect()
}

/// Advise on emitted declarations of a fully checked module. The input IR is
/// unchanged; dead declarations are projected away only in this private copy.
pub fn analyze(source: &str, module: &Module) -> Report {
    let module = crate::link::project(module);
    let live = module
        .functions
        .iter()
        .map(|f| reachable(&module, f))
        .collect::<Vec<_>>();
    let mut copies = vec![None; module.functions.len()];
    loop {
        let mut changed = false;
        for (index, function) in module.functions.iter().enumerate() {
            if copies[index].is_none()
                && let Some(body) = &function.body
                && let Some(origin) = find_copy(body, &copies, &live[index])
            {
                copies[index] = Some(origin);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let memory = crate::regions::analyze(&module);
    let mut report = Report::empty("typed_core");
    for (index, function) in module.functions.iter().enumerate() {
        if let Some(body) = &function.body {
            Visitor {
                source,
                copies: &copies,
                live: &live[index],
                host_allocations: host_allocations(&memory[index], function),
                warnings: &mut report.warnings,
            }
            .visit(body, false);
        }
    }
    report
        .warnings
        .sort_by_key(|d| (d.primary.as_ref().map(|l| l.span), d.code));
    report
        .warnings
        .dedup_by_key(|d| (d.primary.as_ref().map(|l| l.span), d.code));
    report
}
