//! Fixed, typed capability contracts. Arbitrary imports retain unknown effects.
use crate::ir::*;
use crate::{Error, effects::EffectSet};

pub const EVENT_LOOP: &str = "wlt:host/event-loop@0.1.0";
pub const TEXT_IO: &str = "wlt:host/text-io@0.1.0";

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub params: Vec<String>,
    pub result: String,
    pub suspends: bool,
    pub capability: String,
    #[serde(default)]
    pub param_names: Vec<String>,
}

pub fn lookup(function: &Function) -> Option<Binding> {
    if let Some(binding) = &function.import_contract {
        return Some(binding.clone());
    }
    let FunctionKind::Import { module, name } = &function.kind else {
        return None;
    };
    let (params, result, suspends, capability): (&'static [&'static str], _, _, _) =
        match (module.as_str(), name.as_str()) {
            (EVENT_LOOP, "sleep") => (&["i32"], "unit", true, "clock"),
            (EVENT_LOOP, "yield") => (&[], "unit", true, "clock"),
            (EVENT_LOOP, "defer-i64") => (&["i32", "i64"], "(future i64)", false, "clock"),
            (EVENT_LOOP, "ticks") => (&["i32", "i32"], "(stream i64)", false, "clock"),
            (EVENT_LOOP, "fallible-i64") => {
                (&["i32", "i32"], "(future (result i64 i32))", false, "clock")
            }
            (TEXT_IO, "read-lines") => (&[], "(stream (result text i32))", false, "text-input"),
            (TEXT_IO, "write-line") => (
                &["text"],
                "(future (result unit i32))",
                false,
                "text-output",
            ),
            _ => return None,
        };
    Some(Binding {
        params: params.iter().map(|s| (*s).to_owned()).collect(),
        result: result.into(),
        suspends,
        capability: capability.into(),
        param_names: vec![],
    })
}

pub(crate) fn js_name(name: &str) -> String {
    name.split('-')
        .enumerate()
        .map(|(i, word)| {
            if i == 0 {
                word.into()
            } else {
                format!("{}{}", word[..1].to_uppercase(), &word[1..])
            }
        })
        .collect()
}

pub(crate) fn manifest(module: &Module) -> Vec<serde_json::Value> {
    module
        .functions
        .iter()
        .filter_map(|f| {
            let binding = lookup(f)?;
            let FunctionKind::Import { module, name } = &f.kind else {
                unreachable!()
            };
            Some(serde_json::json!({
                "module": module,
                "name": name,
                "jsName": js_name(name),
                "params": binding.params,
                "result": binding.result,
                "capability": binding.capability,
                "effects": { "host": true, "suspends": binding.suspends }
            }))
        })
        .collect()
}

pub fn effects(function: &Function) -> Option<EffectSet> {
    lookup(function).map(|binding| EffectSet {
        host: true,
        suspends: binding.suspends,
        ..EffectSet::default()
    })
}

pub(crate) fn validate(source: &str, module: &Module) -> Result<(), Error> {
    for function in &module.functions {
        let FunctionKind::Import {
            module: namespace, ..
        } = &function.kind
        else {
            continue;
        };
        let family = match namespace.as_str() {
            EVENT_LOOP => "event-loop",
            TEXT_IO => "text-io",
            _ if function.import_contract.is_some() => "policy",
            _ => continue,
        };
        let binding =
            lookup(function).ok_or_else(|| Error::message(format!("unknown {family} binding")))?;
        let params = function
            .params
            .iter()
            .map(|id| module.type_name(&function.locals[id.0].ty))
            .collect::<Vec<_>>();
        if params != binding.params || module.type_name(&function.result) != binding.result {
            return Err(Error::at(
                source,
                Span::default(),
                format!("binding signature mismatch for `{}`", function.name),
            ));
        }
        if function.import_contract.is_some() {
            if !binding.param_names.is_empty() && binding.param_names.len() != params.len() {
                return Err(Error::message(
                    "binding parameter names must match arity".into(),
                ));
            }
            for id in &function.params {
                crate::wit::validate_type(module, &function.locals[id.0].ty)?;
            }
            crate::wit::validate_type(module, &function.result)?;
        }
    }
    Ok(())
}

/// Compiler configuration supplied by the host, never inferred from guest source.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingPolicy {
    pub imports: Vec<PolicyImport>,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PolicyImport {
    pub module: String,
    pub name: String,
    #[serde(flatten)]
    pub binding: Binding,
}
impl BindingPolicy {
    pub fn parse(json: &str) -> Result<Self, Error> {
        let policy: Self = serde_json::from_str(json)
            .map_err(|e| Error::message(format!("binding policy: {e}")))?;
        let mut seen = std::collections::BTreeSet::new();
        for item in &policy.imports {
            if item.binding.capability.is_empty() || !seen.insert((&item.module, &item.name)) {
                return Err(Error::message(
                    "binding policy requires unique imports and nonempty capabilities".into(),
                ));
            }
            if matches!(item.module.as_str(), EVENT_LOOP | TEXT_IO) {
                return Err(Error::message(
                    "binding policy cannot override built-in contracts".into(),
                ));
            }
        }
        Ok(policy)
    }
    pub(crate) fn apply(&self, module: &mut Module) {
        for f in &mut module.functions {
            if let FunctionKind::Import { module, name } = &f.kind
                && let Some(item) = self
                    .imports
                    .iter()
                    .find(|p| &p.module == module && &p.name == name)
            {
                f.import_contract = Some(item.binding.clone());
            }
        }
    }
}
