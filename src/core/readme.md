# Shared semantic core

The simulator, linter and language server share owned frontend-neutral data.
Slang capture enters through `compile` and `Db::from_slang`; consumers neither
retain native pointers nor reopen source paths.

| Component | Responsibility |
| --- | --- |
| `compile/` | Bounded source/include admission, library/configuration mapping, edition checks and diagnostics. |
| `db/` | Validated semantic arena, typed references, source buffers and import/test builders. |
| `value.rs`, `elab.rs` | Exact four-state values, arbitrary-byte strings and pure value operations. |
| `model.rs` | Editor and hierarchy projections. |
| `tokens.rs`, `macros.rs` | Lexical tokens, exact declaration/reference bindings and macro information. |
| `lint/` | Shared rules over the owned DB/design model. |

Library-map selection, source-preserving configuration projection and strict
edition checks are shared by execution and navigation. See the
[database](db/readme.md), [linter](lint/readme.md) and
[source map](../../docs/source_layout.md).
