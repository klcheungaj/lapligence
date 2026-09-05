# core::db

The public `core::db` module is a narrow facade over the owned UHDM snapshot.
`database.rs` owns the arena, shared builder state, and validated `Db::build`
entry point. The private `capture/` hierarchy performs the single canonical
VPI walk by domain concept. `validate.rs` verifies arena roots and side-table
references before `Db::build` returns. Root collections and metadata maps are
private; consumers receive read-only slices, iterators, and checked lookups.

`domain.rs` defines owned enums for integer VPI discriminants. Each enum has an
`Unknown(raw)` variant so newer UHDM values survive capture without acquiring a
false meaning. Capture is grouped into hierarchy, declarations, expressions,
primitives, and statements, with methods visible only inside `core::db`. No
downstream module should interpret integer VPI property values directly.
