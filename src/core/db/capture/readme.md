# core::db::capture

These private modules own the live VPI walk used to build the owned database.
They are grouped by hierarchy, declarations, expressions, primitives, and
statements. Each integer-valued VPI discriminant is converted at the capture
boundary to a `core::db` domain enum; unknown values remain explicit and
retain the original integer for diagnostics and forward compatibility.

`database.rs` owns the arena types, validated `Db::build` facade, and shared
builder state. Capture methods are visible only inside `core::db`. Downstream
consumers use the immutable database accessors and never call VPI directly.
