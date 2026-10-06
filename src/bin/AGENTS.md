# Executables

Keep reusable processing in the library. Bins import `llg::config`, `llg::core`, `llg::ffi` and
`llg::sim`; no `#[path]` includes or `unsafe`. LSP-only dependencies stay in `llg_ls`.

| Binary | Role |
| --- | --- |
| `llg_ls` | tower-lsp stdio server; [guide](llg_ls/AGENTS.md) |
| `llg` | compile → lower/IR/opt/emit → CMake build → run; `llg/` holds `main.rs` (flow), `cli.rs` (parsing) and `settings.rs` (config + precedence) |
| `elab_check` | Owned DB and resolved-hierarchy validation |
| `helloslang`, `helloworld`, `llg_demo` | Owned snapshot demonstrations |

## Simulator driver

- `--generator <backend>` selects CMake `-G`; `--gen-only` writes model sources
  and `CMakeLists.txt` only. CMake is the sole builder;
  [sim](../sim/AGENTS.md) owns compiler/flags/environment selection.
- `--out-dir <dir>` (default `build`) places the model in `<dir>/sim/<design>`.
  `--runtime-cache` > `$LLG_RUNTIME_CACHE_DIR` > `<out-dir>/llg-runtime-cache`;
  `--cc`, `--cflags` and `--cmake` override their environment fallbacks.
  `--model-opt-level <O0|O1|O2|O3|Os>` selects model/runtime C optimization
  (O3 default); later `--cflags`/`LLG_CFLAGS` can override it. Source-only
  projects retain the level.
  `--build-jobs <N>` (positive) > `$CMAKE_BUILD_PARALLEL_LEVEL` > available CPUs
  sets `cmake --build --parallel`. Tool invocation options are ignored with a
  warning under `--gen-only`. Never derive defaults from compile-time paths.
- Experimental model selection uses `LLG_VALUE_BACKEND=legacy|compact` and
  `LLG_COMPACT_KERNELS=portable|gmp`; defaults legacy/portable. GMP kernels
  build the bundled `vendor/gmp` subset; `GMP_ROOT` optionally names an
  installation. The emitter and builder receive the same configuration.
  Compact selection remains experimental; do not claim production support before V12.
- Repeated `--dpi-lib <path>` validates explicit DPI-C libraries before CMake and
  retains them in source-only output. `--no-opt` disables normally enabled IR
  passes; conformance fixtures exercise both modes.
- `--include-dir`/`-I` admits bounded canonical files under source/configured
  include roots. `--define`/`-D <NAME[=VALUE]>` seeds preprocessing before admission,
  including macro-expanded include names. Native reads remain cache-only.
- `--max-export-mib <MiB>` accepts 1–16384, default 4096, and supplies
  `Limits::simulator` to checked compilation, including report-only lint.
  Export exhaustion names the effective budget and the option; native record
  ceilings are fixed. [FFI](../ffi/AGENTS.md) owns the separate library defaults
  and native ceilings. The optional process-memory guard remains independent.
- `--lint` runs shared lint before codegen and exits 1 on errors.
  `--lint-config <path>` loads `llg-lint.toml`. `--lint-json [<path>]` takes
  precedence over `--lint`, emits one JSON object to stdout/file and exits without
  simulation. [Lint](../core/lint/AGENTS.md) owns schema and exit details.
- `--stop-policy <resume|exit>` defaults to same-time coroutine resumption.
  `exit` returns from the child without draining work or running finals; embedders
  also have an explicit runtime resume hook.
- Simulation builds one owned DB using the physical source inventory for bounded
  time-literal recovery and reuses it after lint. Report-only lint uses ordinary
  capture without new constant-source reads.

## Configuration (`llg.toml`)

`llg` reads a config file only from `--config <file>` (must exist); it never
discovers `llg.toml` (`llg_ls` keeps its per-root discovery). Parsing,
validation and path resolution live in `llg::config`, shared with `llg_ls`;
never parse TOML in a bin. `cli.rs` records only what was given; `settings.rs`
resolves command line > environment > config > built-in, and `settings::layered`
is the single place that states that order: every option with an environment
variable goes through it (add new env-backed options there and to the `Env`
reader, not to `sim::build`). Scalars replace; a list option given on the
command line (including source files and `--` plusargs) replaces the config
list, and its `--append-<list>` twin adds to it (replacing values first, then
appended ones; `settings::listed` is the one merge). A later `NAME=VALUE`
define/override replaces an earlier one; other lists keep the first identical
entry; booleans have `--no-X` opposites (`--no-gen-only`, `--opt`, `--no-lint`).
`--config` and `--lint-config` are command-line only. A config error exits 1
naming file and key; entry-level warnings go to stderr. Add a new option to
`cli.rs`, the config schema, `docs/config.md` (a new list needs both its
replace and its `--append-` option, an `Appends` field and a row in the list
table) `docs/config.md` and the resolve/precedence tests together.

## Startup and process state

`--help`/`-h` and `--version`/`-V` print to stdout and succeed before memory guards,
compilation or serving. Version text uses `env!("CARGO_PKG_VERSION")`.

`llg_ls` and `helloworld` install mimalloc. On musl, root build wrapping applies to
every binary, including tests not importing `llg`; carry the shim/mimalloc archive
through library metadata and every final package target. Bins may install
`llg::memory_limit::install[_with_logger]`; [shared policy](../AGENTS.md) owns
native/default/child limits. Keep platform calls in `ffi/process_memory.rs`.
LSP logging uses `LLG_LOG`/`LLG_LOG_FILE`, never serving stdout. `llg_ls` accepts
`--stdio`, `--dump-tokens <PATH>` and `--staging-dir <DIR>` once each, parsed
only in `main.rs`.
