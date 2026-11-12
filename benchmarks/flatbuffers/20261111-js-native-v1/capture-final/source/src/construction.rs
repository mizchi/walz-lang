//! Static field matching, independent of expression checking and evaluation.
use crate::{Error, ir::Field, sexpr::Sexp};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn fields<'a>(
    source: &str,
    constructor: &Sexp,
    fields: &[Field],
    bindings: &'a [Sexp],
) -> Result<Vec<(usize, &'a Sexp)>, Error> {
    let error = |node: &Sexp, message: String| {
        Error::at(source, node.span, message).with_code("E_STRUCT_FIELDS")
    };
    let names = fields.iter().enumerate().map(|(index, field)| {
        field.name.as_deref().map(|name| (name, index)).ok_or_else(|| {
            error(constructor, "named construction requires every field to have a name; use struct.new for anonymous fields".into())
        })
    }).collect::<Result<BTreeMap<_,_>,_>>()?;
    let mut seen = BTreeSet::new();
    let mut plan = Vec::new();
    for binding in bindings {
        let parts = binding.list().filter(|p| p.len() == 2).ok_or_else(|| {
            error(
                binding,
                "expected ($field initializer) in named construction".into(),
            )
        })?;
        let name = parts[0]
            .atom()
            .filter(|n| n.starts_with('$'))
            .ok_or_else(|| error(&parts[0], "expected a named $field".into()))?;
        let index = *names.get(name).ok_or_else(|| {
            error(&parts[0], format!("unknown field `{name}`")).help(format!(
                "available fields: {}",
                names.keys().copied().collect::<Vec<_>>().join(", ")
            ))
        })?;
        if !seen.insert(index) {
            return Err(error(&parts[0], format!("duplicate field `{name}`")));
        }
        plan.push((index, &parts[1]));
    }
    let missing = names
        .iter()
        .filter(|(_, index)| !seen.contains(index))
        .map(|(name, _)| *name)
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(error(
            constructor,
            format!("missing fields: {}", missing.join(", ")),
        ));
    }
    Ok(plan)
}
