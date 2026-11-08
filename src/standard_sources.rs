//! Single catalogue shared by the resolver and documentation generator.
pub(crate) const TYPES: &str = include_str!("../stdlib/types.wlt");
/// Shared dependencies that are not members of the public `wlt:core` facade.
pub(crate) const INTERNAL_LIBRARIES: &[(&str, &str)] =
    &[("json-string.wlt", include_str!("../stdlib/json-string.wlt"))];
pub(crate) const LIBRARIES: &[(&str, &str)] = &[
    ("bytes.wlt", include_str!("../stdlib/bytes.wlt")),
    ("text.wlt", include_str!("../stdlib/text.wlt")),
    ("bytes-view.wlt", include_str!("../stdlib/bytes-view.wlt")),
    ("string-view.wlt", include_str!("../stdlib/string-view.wlt")),
    ("array-view.wlt", include_str!("../stdlib/array-view.wlt")),
    ("list.wlt", include_str!("../stdlib/list.wlt")),
    ("option.wlt", include_str!("../stdlib/option.wlt")),
    ("result.wlt", include_str!("../stdlib/result.wlt")),
    ("dict.wlt", include_str!("../stdlib/dict.wlt")),
    (
        "list-builder.wlt",
        include_str!("../stdlib/list-builder.wlt"),
    ),
    ("builder.wlt", include_str!("../stdlib/builder.wlt")),
    ("json.wlt", include_str!("../stdlib/json.wlt")),
    ("json-value.wlt", include_str!("../stdlib/json-value.wlt")),
];
