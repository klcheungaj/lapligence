# `llg.toml` schema

`llg::config` is the single definition of the `llg.toml` file read by both the
`llg` simulator driver and the `llg_ls` language server. The user-facing key
reference, per-key tool applicability, discovery and precedence rules are in
[configuration](../../docs/config.md).

| File | Responsibility |
| --- | --- |
| `mod.rs` | Resolved types (`LlgConfig` and per-table structs), constants, defaults, `include_dirs`. |
| `schema.rs` | Raw serde tables; every table denies unknown fields. |
| `resolve.rs` | Validation, path resolution against the config directory, fail-soft entries, `discover_sources`. |
| `load.rs` | Bounded file read, `parse_config_detailed`/`load_config_file`, key-and-line error context. |
| `paths.rs` | Lexical path normalization, glob matching and source discovery shared with the server. |
| `tests.rs` | Schema, resolution and loading tests. |

Design notes:

- Every key is valid for both tools; a key a tool does not use is validated and
  ignored there, so one file serves editor and simulator and a misspelled key
  rejects the whole file atomically in either.
- Optional driver settings stay `None` when unset, so the driver can apply
  command line > config > environment/built-in precedence (see `src/bin/llg/settings.rs`).
- `toml` and `serde` are regular dependencies; the schema is not behind the
  `lsp` feature because the simulator driver needs it too.
- This module may depend on `core` and `sim` (for validated enums such as
  `ModelOptLevel`), never the reverse; it holds no process state and performs no
  native calls.
