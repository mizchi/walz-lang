//! Small typed rewrites. Full source validation precedes this compiler TCB pass.
use crate::ir::*;
mod inline;
mod range_facts;
mod ranges;

mod values;

pub(crate) fn simplify(module: &Module) -> Module {
    ranges::simplify(&values::simplify(module))
}
