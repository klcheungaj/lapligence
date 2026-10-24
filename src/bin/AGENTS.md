# Agent guide: src/bin

Thin executables over the library. Roles: [readme](readme.md); user CLI:
[readme](../../readme.md#using-llg) and [configuration](../../docs/config.md).

## Directory map

- `llg/`: `main.rs` (flow; module docs list every option, the lint output and
  exit codes), `cli.rs` (parsing only), `settings.rs` (config and precedence).
- `llg_ls/`: language server ([guide](llg_ls/AGENTS.md)).
- `elab_check.rs`: owned DB and resolved-hierarchy validation.
- `helloslang.rs`, `helloworld.rs`, `llg_demo.rs`: snapshot demonstrations.

## Rules

- Keep reusable processing in the library. Bins import `llg::config`,
  `llg::core`, `llg::ffi` and `llg::sim`; no `#[path]` includes, no `unsafe`.
- LSP-only dependencies stay in `llg_ls` behind the `lsp` feature.
- Never parse TOML in a bin: `llg::config` owns parsing, validation and paths.
- `llg` reads a config only from `--config`; it never discovers `llg.toml`.
- `settings::layered` is the single statement of command line > environment >
  config > built-in; every env-backed option goes through it and the `Env`
  reader, not `sim::build`. `settings::listed` is the one list merge.
- Never derive defaults from compile-time paths.
- `sim` owns compiler, flag, generator and environment selection; the FFI owns
  native limits (`llg` compiles with `Limits::simulator()`).
- Models always use compact values with GMP kernels. `LLG_DEV_VALUE_BACKEND` and
  `LLG_DEV_COMPACT_KERNELS` are development-only parity selectors: never
  document them for users.
- Parameter-override typing belongs to the wrapper
  ([readme](../wrapper/readme.md#parameter-overrides)); `llg.toml` floats and
  booleans become exact literals in `llg::config`.
- Help and version print to stdout and succeed before memory guards, compilation
  or serving. LSP logging never writes to serving stdout.
- On musl, root build wrapping applies to every binary, including tests not
  importing `llg`; carry the shim/mimalloc archive through library metadata and
  every final package target.

## Adding an option

Change together: `cli.rs`, the config schema, `docs/config.md` (a new list needs
its replace option, its `--append-` twin, an `Appends` field and a row in the
list table), the user readme, and the resolve/precedence tests.

## Pitfalls

- Simulation builds one owned DB (with the physical source inventory for bounded
  time-literal recovery) and reuses it after lint; report-only lint uses ordinary
  capture without new constant-source reads.
- Unknown `-` options are usage errors (exit 2), never source files.
