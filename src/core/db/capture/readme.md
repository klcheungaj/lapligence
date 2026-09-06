# Database capture

- Purpose: private live-VPI traversal behind `core::db::Db::build`.
- Modules: hierarchy, declarations, expressions, primitives, and statements.
- Output: owned arena nodes and domain enums; unknown discriminants stay explicit.
- Boundary: capture methods stay inside `core::db`; consumers use database accessors.
- Related: [owned database](../readme.md) and [safe FFI layer](../../../ffi/readme.md).
