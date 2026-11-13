//! Checked Component connection contracts. All IDs refer to one decoded graph,
//! so alias equality is preserved without comparing only resource spellings.
use std::collections::BTreeMap;
pub(crate) struct BoundImports {
    pub resolve: wit_parser::Resolve,
    pub interfaces: BTreeMap<String, wit_parser::InterfaceId>,
    pub functions: BTreeMap<String, BTreeMap<String, wit_parser::Function>>,
    pub types: BTreeMap<String, wit_parser::Type>,
}
