//! Single catalogue shared by the resolver and documentation generator.
pub(crate) const TYPES: &str = include_str!("../walz/core/types.walz");
/// Shared dependencies that are not members of the public `walz:core` facade.
pub(crate) const INTERNAL_LIBRARIES: &[(&str, &str)] = &[(
    "json-string.walz",
    include_str!("../walz/core/json-string.walz"),
)];
pub(crate) const LIBRARIES: &[(&str, &str)] = &[
    ("bytes.walz", include_str!("../walz/core/bytes.walz")),
    ("text.walz", include_str!("../walz/core/text.walz")),
    (
        "bytes-view.walz",
        include_str!("../walz/core/bytes-view.walz"),
    ),
    (
        "string-view.walz",
        include_str!("../walz/core/string-view.walz"),
    ),
    (
        "array-view.walz",
        include_str!("../walz/core/array-view.walz"),
    ),
    ("list.walz", include_str!("../walz/core/list.walz")),
    ("option.walz", include_str!("../walz/core/option.walz")),
    ("result.walz", include_str!("../walz/core/result.walz")),
    ("dict.walz", include_str!("../walz/core/dict.walz")),
    (
        "list-builder.walz",
        include_str!("../walz/core/list-builder.walz"),
    ),
    ("builder.walz", include_str!("../walz/core/builder.walz")),
    ("json.walz", include_str!("../walz/core/json.walz")),
    (
        "json-value.walz",
        include_str!("../walz/core/json-value.walz"),
    ),
];
