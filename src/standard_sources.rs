//! Single catalogue shared by the resolver and documentation generator.
pub(crate) const TYPES: &str = include_str!("../stdlib/types.wlt");
pub(crate) const LIBRARIES: &[(&str, &str)] = &[
    ("bytes.wlt", include_str!("../stdlib/bytes.wlt")),
    ("text.wlt", include_str!("../stdlib/text.wlt")),
    ("list.wlt", include_str!("../stdlib/list.wlt")),
    ("dict.wlt", include_str!("../stdlib/dict.wlt")),
    (
        "list-builder.wlt",
        include_str!("../stdlib/list-builder.wlt"),
    ),
    ("builder.wlt", include_str!("../stdlib/builder.wlt")),
    ("json.wlt", include_str!("../stdlib/json.wlt")),
    ("json-value.wlt", include_str!("../stdlib/json-value.wlt")),
];
