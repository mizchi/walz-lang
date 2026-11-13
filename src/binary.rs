//! Binary-safe WAT string literals.
pub(crate) fn quoted(bytes: &[u8]) -> String {
    format!(
        "\"{}\"",
        bytes
            .iter()
            .map(|b| format!("\\{b:02x}"))
            .collect::<String>()
    )
}
