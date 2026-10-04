# llg.toml — Configuration for `llg` and `llg_ls`

One TOML file (schema `1`) drives both tools: the `llg` simulator driver and
the `llg_ls` language server. Its keys follow `llg`'s command-line options,
written as snake_case keys grouped in tables, with arrays for repeatable
options. A key only one tool uses is still accepted, validated and ignored by
the other (the tables below say which tool uses each key), so a single file
can serve a project's editor and its simulation runs. The schema is implemented
once, in the library (`llg::config`), and both binaries use it.

## Discovery and paths

- **`llg`**: `--config <path>` names the file explicitly (relative to the
  current directory); a missing explicit file is an error. Otherwise
  `llg.toml` in the **current directory** is read when present; a missing
  default file is not an error. There is no search through parent directories:
  the driver acts on the directory it is started in, so the file that applies
  is always the visible one. With no arguments and no `llg.toml`, `llg` prints
  usage and exits 2.
- **`llg_ls`**: every workspace root is an independent analysis root with its
  own effective `llg.toml`, loaded from the root directory unless the client
  overrides the path via `initializationOptions`
  (`{ "llg": { "protocolVersion": 1, "configFiles": [{ "workspaceUri": …,
  "path": … }] } }`). `llg_ls --dump-tokens <PATH>` uses the same loader: the
  `llg.toml` of the directory (or of the nearest ancestor of the file) given.
  A missing file uses safe defaults (root as the sole source directory,
  recursive `.v`/`.sv`, built-in excludes). The server watches the file and any
  parsed-config change hot-reloads the root, with no restart.
- Relative paths inside the file resolve from the directory containing it;
  absolute paths are accepted. Program names (`build.cc`, `build.cmake`,
  `build.launcher`, `build.generator`) and flag strings are not paths.
  Defaults that the file does not set keep their command-line meaning (for
  example the output directory `build` is relative to the current directory).

## Precedence in `llg`

Highest first: **command line, `llg.toml`, environment fallbacks, built-in
defaults.** The file supplies defaults for the command-line options, so it
ranks exactly where the option would: above `$LLG_CC`, `$LLG_CFLAGS`,
`$LLG_C_LAUNCHER`, `$LLG_CMAKE`, `$LLG_RUNTIME_CACHE_DIR`, `$CMAKE_BUILD_PARALLEL_LEVEL` and
`$CMAKE_GENERATOR` (use the option or an unset key to let the environment
decide).

- A scalar given on the command line replaces the file's value.
- A **repeatable option given on the command line replaces the file's whole
  list** for that option; it never appends. (`-I`, `-D`, `--param-override`,
  `--define-system-task`, `--libmap`, `--libfile`, `--library-order`,
  `--dpi-lib`, the plusargs after `--`.) An explicit `--` with nothing after it
  clears the file's `simulator.plusargs`.
- Source files named on the command line replace **all** of the file's
  sources (`sources.files` and the `sources.directories` discovery); the
  source directories then also stop being include directories. Without
  command-line files, the sources are `sources.files` plus every `.v`/`.sv`
  found under `sources.directories` (only when the file names directories
  explicitly; the language server's implicit `["."]` default does not make
  `llg` scan the directory) with `sources.include`/`exclude`. Source
  directories are include-search directories, as in the language server;
  `-I` replaces only `compile.include_dirs`.
- Booleans have explicit opposites so the command line can override either
  value: `--gen-only`/`--no-gen-only`, `--no-opt`/`--opt`, `--lint`/`--no-lint`.
  `--no-lint` also cancels `lint.json`. `--lint-json [<path>]` chooses its own
  destination (stdout without a path) instead of the file's `lint.json_file`.
- `--lint-config <file>` (the legacy `llg-lint.toml` rule file) replaces the
  `[lint]` rule settings of `llg.toml`; without it, the `[lint]` rules
  (`enabled`, `rules.<id>`) of `llg.toml` apply to `llg --lint`.
- `--help` and `--version` act before any file is read. `--config` and
  `--lint-config` are command-line only.

## Errors

A config problem stops `llg` with exit code 1 and a message naming the file
and key, for example
`llg: invalid config /proj/llg.toml: invalid llg.toml at line 3, key
`compile.topp`: unknown field `topp`, expected one of …`. Unknown keys, an
unknown `schema_version`, wrong types and out-of-range values reject the whole
file atomically in both tools; a misspelled key cannot silently change a run
or an analysis. Entry-level problems (a malformed `defines` entry, an invalid
`param_overrides` name) are dropped with a warning (`llg` prints
`llg: warning: <file>: …` to stderr; the server publishes them against the TOML
URI). Missing configured directories also warn. The file read is bounded to
1 MiB.

In the language server a rejected file produces one diagnostic per error on
the TOML URI whose message is `invalid <path>: <error>` (key and line
included); the previous valid configuration stays in effect on reload, and safe
defaults apply until a valid file exists.

## Key reference

Tool column: **both** = used by `llg` and `llg_ls`; **llg** = used by the
driver, accepted and ignored by `llg_ls`; **ls** = used by the language
server, accepted and ignored by `llg`.

| Key | Type | Tool | Notes |
|---|---|---|---|
| `schema_version` | integer | both | required, must be `1` |
| `sources.directories` | strings | both | source directories; `llg` discovers only when set |
| `sources.include` / `sources.exclude` | globs | both | discovery filters |
| `sources.files` | strings | llg | explicit source files (`<file.sv>...`) |
| `compile.top` | string | both | `--top` (`llg` also takes `module:config`) |
| `compile.edition` | `"2001"` \| `"2009"` | llg | `--edition`; the server compiles 2009 |
| `compile.compilation_units` | `"separate"` \| `"merged"` | llg | `--compilation-units`; the server always separates |
| `compile.include_dirs` | strings | both | `-I` |
| `compile.defines` | `NAME[=VALUE]` strings | both | `-D` |
| `compile.param_overrides` | table | both | `--param-override`/`-G`, `NAME = "value"` or integer |
| `compile.system_tasks` | strings | llg | `--define-system-task` prototypes |
| `libraries.map_files` | strings | llg | `--libmap` |
| `libraries.files` | `[lib=]path` strings | llg | `--libfile`; only the path is resolved |
| `libraries.order` | strings | llg | `--library-order` |
| `libraries.default` | string | llg | `--default-library` |
| `lint.enabled`, `lint.rules.<id>` | | both | rule switches/severities (`llg` applies them with `--lint`) |
| `lint.run` | bool | llg | `--lint` |
| `lint.json` | bool | llg | `--lint-json` (stdout) |
| `lint.json_file` | string | llg | `--lint-json <path>`; implies `json` |
| `analysis.max_file_bytes`, `analysis.max_total_input_bytes` | integers | ls | input budgets |
| `simulator.stop_policy` | `"resume"` \| `"exit"` | llg | `--stop-policy` |
| `simulator.max_export_mib` | 1 to 16384 | llg | `--max-export-mib` |
| `simulator.optimize` | bool | llg | `--no-opt` is `optimize = false` |
| `simulator.plusargs` | strings | llg | arguments after `--` |
| `build.gen_only` | bool | llg | `--gen-only` |
| `build.generator`, `build.launcher`, `build.cc`, `build.cmake` | strings | llg | `--generator`, `--launcher`, `--cc`, `--cmake` (`build.launcher` falls back to `$LLG_C_LAUNCHER`) |
| `build.cflags` | string | llg | `--cflags` (empty clears `$LLG_CFLAGS`) |
| `build.model_opt_level` | `O0` `O1` `O2` `O3` `Os` | llg | `--model-opt-level` |
| `build.jobs` | positive integer | llg | `--build-jobs` |
| `build.dpi_libs` | strings | llg | `--dpi-lib` |
| `output.out_dir` | string | llg | `--out-dir` |
| `output.runtime_cache` | string | llg | `--runtime-cache` |

Command-line-only: `--config`, `--lint-config`, `--help`, `--version`, and the
negations `--no-gen-only`, `--opt`, `--no-lint`.

```toml
schema_version = 1

[sources]
directories = ["rtl", "tb"]      # llg runs every .v/.sv found here
exclude = ["**/generated/**"]

[compile]
top = "tb"
include_dirs = ["../common/includes"]
defines = ["WIDTH=8", "ENABLE_SIM"]

[compile.param_overrides]
W = 16

[lint]
run = true

[simulator]
stop_policy = "exit"

[build]
model_opt_level = "O2"
jobs = 8

[output]
out_dir = "build/sim_out"
```

## `schema_version`

Required integer; must be `1`.

## `[sources]` — source discovery

- `directories` (array of strings) — source directories; each is also an
  include-search directory. Default `["."]` for `llg_ls`; may point outside the
  workspace. `llg` discovers sources here only when the key is set.
- `files` (array of strings, `llg` only) — explicit source files, the
  equivalent of the command-line `<file.sv>...`. Ignored by `llg_ls`, which
  discovers sources from `directories`.
- `include` (array of globs) — root-relative include globs evaluated against
  each source directory. Only `.v`/`.sv` files become compilation units
  (`.vh`/`.svh` and other extensions enter analysis only through include
  resolution). Default `["**/*.v", "**/*.sv"]`.
- `exclude` (array of globs) — excludes win over includes. Built-in defaults:
  `slpp_all/**`, `.git/**`, `target/**`.

## `[compile]` — compilation inputs

- `top` (string) — elaboration top module (auto-detected when omitted); `llg`
  also accepts `module:config`.
- `edition` (`"2001"` or `"2009"`, `llg` only) — language edition
  (`--edition`, default 2009). `llg_ls` always compiles the 2009 edition.
- `compilation_units` (`"separate"` or `"merged"`, `llg` only) — compilation-unit
  grouping (`--compilation-units`, default `separate`). `merged` preserves each
  source buffer's identity while sharing preprocessing and `$unit` scope;
  library sources group per library, in admission order, separately from work
  sources. `llg_ls` always uses `separate`.
- `include_dirs` (array of strings) — additional include-search directories
  (may be external); source directories are already include dirs.
- `defines` (array of `NAME` or `NAME=VALUE`) — preprocessor defines applied
  to every analyzed source (`-D`); they drive `` `ifdef ``/`` `elsif ``
  selection and macro expansion.
- `param_overrides` (table, `NAME = <string | integer>`) — top-level parameter
  overrides (`-PNAME=VALUE`, equivalent to `top -GNAME=value`); they apply to
  the top-level instances only, and an override no top module declares is
  reported as an error. `llg` accepts the same overrides with
  `--param-override NAME=VALUE` (`-G`).
- `system_tasks` (array of strings, `llg` only) — VPI system task/function
  prototypes (`--define-system-task`).

Include roots are canonicalized and bounded before Slang sees them; a
macro-expanded include that resolves outside the source file's parent or these
explicit roots is left to the cache-only frontend and therefore produces a
source diagnostic without reading the host path.

## `[libraries]` — library admission (`llg` only)

- `map_files` (array of strings) — library map files (`--libmap`).
- `files` (array of `[library=]path`) — explicit library sources
  (`--libfile`); only the path part is resolved against the config directory.
- `order` (array of library names) — default configuration search order
  (`--library-order`).
- `default` (string) — name of the default source library (`--default-library`,
  default `work`).

## `[lint]` — linter configuration

- `enabled` (bool) — global switch; `false` disables every rule (a per-rule
  entry can re-enable individual rules).
- `rules.<id>` (table per rule) — `enabled` (bool) and `severity`
  (`"error"` | `"warning"` | `"info"`). Unknown rule ids are errors. `llg`
  applies `enabled` and `rules` when linting (`--lint`), unless
  `--lint-config` supplies a rule file instead.
- `run` (bool, `llg` only) — run the linter before simulation (`--lint`).
- `json` (bool, `llg` only) — report lint as JSON on stdout and exit
  (`--lint-json`); implies `run`.
- `json_file` (string, `llg` only) — write the JSON report to this file
  (`--lint-json <path>`); implies `json`.

Default rules (enabled; find `[lint.rules.<id>]` snippets below):

| Rule id | What it flags |
|---|---|
| `unused-signal` | signals that are never read |
| `width-mismatch` | assignments/port links whose known widths differ |
| `incomplete-case` | `case` without `default` in combinational/latch processes |
| `combinational-loop` | combinational feedback paths |
| `multi-driver` | signals driven from multiple processes |
| `casez-misuse` | `casez`/`casex` misuse of `?` as don't-care |
| `if-latch` | `if` without `else` implying a latch |
| `naming-style` | naming convention violations |
| `blocking-in-always_ff` | blocking assignment inside `always_ff` |
| `nba-in-always_comb` | non-blocking assignment inside `always_comb` |
| `unused-parameter` | parameters that are never used |
| `implicit-net` | nets auto-created from undeclared identifiers |
| `case-default-missing` | `case`/`casex`/`casez` without `default` elsewhere |
| `comparison-width-mismatch` | comparisons (`==`, `<`, …) with differing known widths |
| `unconnected-port` | instance ports left unconnected |
| `mixed-assignments` | blocking and non-blocking assignments in one process |
| `undriven-signal` | signals that are read but have no known source |
| `incomplete-sensitivity-list` | explicit level-sensitive blocks missing a body input |
| `out-of-range-select` | statically provable packed or unpacked select overflow |
| `xz-logical-equality` | `==`/`!=` used directly with an X/Z literal |
| `duplicate-case-item` | repeated literal labels in an exact `case` |
| `empty-implicit-sensitivity` | `always @*` blocks that write but have no resolved body reads |
| `assignment-in-condition` | assignment expressions used as truth predicates |
| `casex-statement` | `casex` statements, which can hide X/Z selector mistakes |

```toml
schema_version = 1

[sources]
directories = ["rtl", "tb"]
include = ["**/*.v", "**/*.sv"]
exclude = ["**/generated/**"]

[compile]
top = "top"
include_dirs = ["../common/includes"]
defines = ["WIDTH=8", "ENABLE_SIM"]

[compile.param_overrides]
W = 16

[analysis]
max_file_bytes = 1048576
max_total_input_bytes = 8388608

[lint]
enabled = true

[lint.rules.unused-signal]
enabled = false

[lint.rules.width-mismatch]
severity = "error"
```

## `[simulator]` — simulation behavior (`llg` only)

- `stop_policy` (`"resume"` or `"exit"`) — `$stop` handling (`--stop-policy`,
  default `resume`).
- `max_export_mib` (integer 1 to 16384) — frontend export budget for the
  elaborated design (`--max-export-mib`, default 4096).
- `optimize` (bool) — simulator IR optimization passes; `false` is `--no-opt`.
- `plusargs` (array of strings) — arguments passed to the generated simulator
  (the arguments after `--`).

## `[build]` — model build (`llg` only)

- `gen_only` (bool) — emit the model sources without building (`--gen-only`).
- `generator`, `launcher`, `cc`, `cmake` (strings) — CMake generator, C compiler
  launcher, C compiler and CMake program. Programs are looked up by name, not
  resolved against the config directory. An unset launcher falls back to
  `$LLG_C_LAUNCHER`.
- `cflags` (string) — extra C compiler flags (`--cflags`); an empty string
  clears `$LLG_CFLAGS`.
- `model_opt_level` (`"O0"`, `"O1"`, `"O2"`, `"O3"`, `"Os"`) — model/runtime C
  optimization (`--model-opt-level`, default `O3`).
- `jobs` (positive integer) — parallel compile jobs (`--build-jobs`).
- `dpi_libs` (array of strings) — explicit DPI-C libraries (`--dpi-lib`).

## `[output]` — output locations (`llg` only)

- `out_dir` (string) — output root; the model goes to `<out_dir>/sim/<design>`
  (`--out-dir`, default `build`).
- `runtime_cache` (string) — runtime archive cache (`--runtime-cache`).

## `[analysis]` — input-size safeguards (`llg_ls` only)

- `max_file_bytes` (positive integer) — maximum UTF-8 buffer or on-disk byte
  length of one unique compilation unit or resolved include, including a
  bounded macro-expanded filename. Default
  `1048576` (1 MiB).
- `max_total_input_bytes` (positive integer) — maximum sum of the measured
  unique compilation units and resolved includes in one analysis, including
  bounded macro-expanded filenames.
  Canonicalized paths are counted once, including include cycles. Default
  `8388608` (8 MiB).

Open UTF-8 buffer text is measured in preference to disk metadata. Discovery
is unchanged, so files over either limit remain watched; the root analysis is
rejected before Slang compilation and publishes an `input-size-limit`
diagnostic while retaining the last-good snapshot. Every discovered/root
compilation unit must also produce a bounded UTF-8 snapshot; a missing or
unreadable root produces an `input-snapshot` diagnostic and is never passed to
Slang on its live path. Open-buffer admission applies the same positive
per-file bound before storing or scheduling the buffer, using the built-in
default until a root-specific config is available. A rejected buffer is not
used by a later compile.

Source-size rejections and native frontend capture-limit failures emit `ERROR`
logs, including when a compact navigation snapshot can be recovered. The log
and diagnostic advise excluding generated or unnecessary paths with
`[sources].exclude`. For source-size limits, you can instead raise the relevant
`[analysis]` budget if memory permits. `LLG_MEMORY_LIMIT_MB` controls a separate
process-memory ceiling: raising it alone does not raise source/export caps.
Native frontend caps are currently server-side limits, not `llg.toml` settings.

Literal includes are resolved beside the including file first, then in the
configured source directories followed by `compile.include_dirs`, in the same
order used for admission. A readable closed input is bounded-read once during
admission, and that exact text is supplied to Slang as an owned source buffer.
The frontend resolves includes only from admitted buffers. Missing or
macro-generated includes that were not admitted produce diagnostics without
reading an unmeasured file. Open buffers use their in-memory contents.

Config reloads are bounded independently of the analysis budgets: at most
`MAX_CONFIG_BYTES` (1 MiB) plus one byte is read from `llg.toml`. Oversized and
invalid-UTF-8 config files are reported as bounded `io::Error` load failures
and do not replace the last valid configuration.

## LSP analysis snapshots

Large elaborated designs use a compact source-navigation snapshot when full
capture exceeds 100,000 semantic nodes or another frontend limit. Tokens are
stored per source file, module bodies and generate blocks are visited once,
and references target source definitions. Hover, definition, references,
symbols, semantic tokens, token dumps, and the module explorer remain
available. The server reports this reduced mode explicitly; custom lint and
instance-specific parameter values, widths, and elaborated hierarchy are not
available in that snapshot. A previous full snapshot remains preferred after
a failing edit. Simulator compilation retains its independent capture limits.

## LSP diagnostic logging

Language-server logging uses stderr or `LLG_LOG_FILE`; stdout is reserved
for framed JSON-RPC messages. Slang diagnostics are captured through the
frontend bridge and published by Rust.

At `LLG_LOG=trace`, the transport wrapper also records every decoded request
or notification (`event=transport.receive`) and its completed dispatch
(`event=transport.dispatch.end`). This distinguishes time spent in JSON-RPC
transport/handler dispatch from time spent in the asynchronous root job. The
method, id, parameter-presence flag, and outcome are bounded and escaped.
