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

Library-map selection, source-preserving native map configuration parsing and strict
edition checks are shared by execution and navigation. See the
[database](db/readme.md), [linter](lint/readme.md) and
[source map](../../docs/source_layout.md).

Per-library `-incdir` in a map adds ordered search prefixes for that library.
Paths are relative to the containing map; Rust admits reachable headers before
Slang's cache-only lookup. Local source directory, global command-line include
directories, then library directories is the selected precedence. Wildcard directory
matches are sorted; logical directories are represented by admitted buffers
under their path, so an empty logical directory cannot be selected.

Macro and conditional expansion supplies the bounded Rust map resolver with the
declarations Slang parses from each original map buffer. This keeps configuration
diagnostics at macro uses. Command-line defines seed each map; a map's local
definitions stay within that map, including when another map is named by a map
`include` declaration. Design source units have separate macro environments.
