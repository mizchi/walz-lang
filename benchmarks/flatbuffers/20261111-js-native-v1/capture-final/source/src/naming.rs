//! Public WIT-oriented vocabulary, separate from stable checked IR/policy names.
use std::borrow::Cow;

const FAMILIES: &[(&str, &str)] = &[("string", "text"), ("tuple", "pair"), ("box", "own")];

fn family<'a>(name: &'a str, reverse: bool) -> Cow<'a, str> {
    for &(public, core) in FAMILIES {
        let (from, to) = if reverse {
            (core, public)
        } else {
            (public, core)
        };
        if name == from {
            return Cow::Borrowed(to);
        }
        if let Some(suffix) = name.strip_prefix(from).filter(|s| s.starts_with('.')) {
            return Cow::Owned(format!("{to}{suffix}"));
        }
    }
    Cow::Borrowed(name)
}

pub(crate) fn core(name: &str) -> Cow<'_, str> {
    family(name, false)
}
pub(crate) fn public(name: &str) -> Cow<'_, str> {
    family(name, true)
}

/// Only the frozen-input loader uses old standard imports. Local aliases survive.
pub(crate) fn legacy_member<'a>(kind: &str, name: &'a str, old_json_object: bool) -> Cow<'a, str> {
    if kind == "type" {
        return match name {
            "$text" => "$string",
            "$pair" => "$tuple",
            "$own" => "$box",
            "$resource" => "$resource-handle",
            "$bytes-view.View" => "$bytes-view",
            "$string-view.View" => "$string-view",
            "$dict.Dict" => "$dict",
            "$list-builder.Builder" => "$list-builder",
            "$builder.Builder" => "$string-builder",
            "$json.Object" => "$json-object",
            "$json-value.Value" => "$json-value",
            "$Kind" => "$json-kind",
            _ => return Cow::Borrowed(name),
        }
        .into();
    }
    if kind == "func" {
        if let Some(suffix) = name.strip_prefix("$json-value.") {
            return format!(
                "$json.{}",
                if suffix == "string" {
                    "from-string"
                } else {
                    suffix
                }
            )
            .into();
        }
        if let Some(suffix) = name.strip_prefix("$json-object.") {
            return format!("$json.object-builder.{suffix}").into();
        }
        if let Some(suffix) = name.strip_prefix("$json.")
            && (matches!(
                suffix,
                "new" | "finish" | "string" | "u64" | "s64" | "value"
            ) || old_json_object && matches!(suffix, "null" | "boolean" | "number" | "object"))
        {
            return format!("$json.object-builder.{suffix}").into();
        }
        for (old, new) in [("$buffer.", "$bytes."), ("$builder.", "$string-builder.")] {
            if let Some(suffix) = name.strip_prefix(old) {
                return format!("{new}{suffix}").into();
            }
        }
    }
    Cow::Borrowed(name)
}
