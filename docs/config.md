# llg.toml — Configuration for `llg` and `llg_ls`

One TOML file (schema `1`) drives both tools: the `llg` simulator driver and
the `llg_ls` language server. Its keys follow `llg`'s command-line options,
written as snake_case keys grouped in tables, with arrays for repeatable
options. A key only one tool uses is still accepted, validated and ignored by
the other (the tables below say which tool uses each key), so a single file
can serve a project's editor and its simulation runs. The schema is implemented
once, in the library (`llg::config`), and both binaries use it.

## Discovery and paths

- **`llg`**: reads a config file **only** when `--config <path>` names it
  (relative to the current directory); a missing explicit file is an error.
  `llg` never discovers `llg.toml`: a file in the current directory (or any
  parent) is ignored unless passed explicitly, so a run never depends on an
  invisible file. With no arguments, `llg` prints usage and exits 2.
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

Highest first: **command line, environment, `llg.toml`, built-in default.**
The merge code expresses the order in one place (`settings::layered`). Every
option that has an environment variable follows it:

| Option | Command line | Environment | Config key | Default |
|---|---|---|---|---|
| C compiler | `--cc` | `$LLG_CC`, then `$CC` | `build.cc` | `cl` (MSVC) on Windows, `cc` elsewhere |
| C flags | `--cflags` | `$LLG_CFLAGS` | `build.cflags` | none |
| CMake program | `--cmake` | `$LLG_CMAKE` | `build.cmake` | `cmake` |
| CMake generator | `--generator` | `$CMAKE_GENERATOR` | `build.generator` | CMake's own |
| C compiler launcher | `--launcher` | `$LLG_C_LAUNCHER` | `build.launcher` | none |
| Build jobs | `--build-jobs` | `$CMAKE_BUILD_PARALLEL_LEVEL` | `build.jobs` | available CPUs |
| Runtime cache | `--runtime-cache` | `$LLG_RUNTIME_CACHE_DIR` | `output.runtime_cache` | `<out-dir>/llg-runtime-cache` |

An empty `$LLG_CC`, `$CC`, `$LLG_CMAKE`, `$CMAKE_GENERATOR`, `$LLG_C_LAUNCHER` or
`$LLG_RUNTIME_CACHE_DIR` counts as unset, and so does a
`$CMAKE_BUILD_PARALLEL_LEVEL` that is not a positive integer. An empty
`$LLG_CFLAGS` is a value (no extra flags) and beats `build.cflags`; the same
holds for `--cflags ""`. All other options (`--model-opt-level`,
`--stop-policy`, ...) have no environment variable: command line, config,
default. `LLG_MEMORY_LIMIT_MB` is an environment-only process guard with no
option or key, so no precedence applies to it.

- A scalar from a higher layer replaces the lower one.
- **List options replace; `--append-<list>` options add.** A list option
  given on the command line replaces the whole config list (several
  occurrences accumulate among themselves, as before the config file existed).
  Its `--append-<list>` twin keeps the config list and adds its values after it.
  Given together for one list, the replacing values are the base (the config
  list is dropped) and the appended values follow them; the order of the two
  kinds on the command line does not matter. Without a config file the
  appended values are the whole list. No list has an environment variable, so
  only the config supplies the lower layer.

| List | Config key | Replace | Append |
|---|---|---|---|
| Source files | `sources.files` and `sources.directories` discovery | `<file.sv>...` | `--append-source <file>` |
| Include directories | `compile.include_dirs` | `-I`, `--include-dir` | `--append-include-dir <path>` |
| Defines | `compile.defines` | `-D`, `--define` | `--append-define <NAME[=VALUE]>` |
| Parameter overrides | `compile.param_overrides` | `-G`, `--param-override` | `--append-param-override <NAME=VALUE>` |
| System tasks | `compile.system_tasks` | `--define-system-task` | `--append-define-system-task <prototype>` |
| Library maps | `libraries.map_files` | `--libmap` | `--append-libmap <file>` |
| Library files | `libraries.files` | `-v`, `--libfile` | `--append-libfile <[library=]file>` |
| Library order | `libraries.order` | `-L`, `--library-order` | `--append-library-order <library>[,<library>...]` |
| DPI libraries | `build.dpi_libs` | `--dpi-lib` | `--append-dpi-lib <path>` |
| Plusargs | `simulator.plusargs` | `--` followed by the arguments | `--append-plusarg <arg>` |

- **Sources and plusargs.** Source files named on the command line replace
  `sources.files` and the discovery under `sources.directories` (and those
  directories stop being include directories); `--append-source` adds files
  after whatever sources apply. `--` replaces the configured plusargs even
  when nothing follows it (it then empties them); `--append-plusarg` adds one
  plusarg after the configured ones, or after the replacing ones when both are
  given. Everything after `--` is a plusarg, so put `--append-plusarg` before
  it.
- **Duplicates.** A define (`NAME` or `NAME=VALUE`) or parameter override
  given later replaces an earlier one with the same `NAME`, so an appended
  `--append-define LEVEL=2` overrides the file's `LEVEL=1`, and the last of
  repeated values wins. Every other list keeps the first occurrence of an
  identical entry (include directories and library names compare as text,
  source files by canonical path). Plusargs are never deduplicated.
- Sources: the config contributes `sources.files` plus every `.v`/`.sv`
  found under `sources.directories` (only when the file names directories
  explicitly; the language server's implicit `["."]` default does not make
  `llg` scan the directory) with `sources.include`/`exclude`. Source
  directories are include-search directories, as in the language server; `-I`
  replaces only `compile.include_dirs`, not the source directories.
- Booleans have explicit opposites so the command line can override either
  value: `--gen-only`/`--no-gen-only`, `--no-opt`/`--opt`,
  `--lint-only`/`--no-lint-only`, `-Werror`/`-Wno-error`. `--no-lint-only` also
  cancels `lint.json`. `--lint-json [<path>]` chooses its own destination
  (stdout without a path) instead of the file's `lint.json_file`.
- `--wave <file>` replaces `waveform.file` and `--no-wave` cancels it;
  `--wave-depth` replaces `waveform.depth` and needs a file from either layer.
- Every `llg` run lints. The `[lint]` rule settings (`enabled`, `rules.<id>`)
  of the `--config` file apply to it; without `--config` every rule runs with
  its default severity.
- `--help` and `--version` act before any file is read. `--config` is
  command-line only.

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
| `compile.top` | module name | both | `--top`; no `library.` prefix or `:config` suffix |
| `compile.edition` | `"v2001"` \| `"sv2009"` | llg | `--edition`; the server compiles sv2009 |
| `compile.compilation_units` | `"separate"` \| `"merged"` | llg | `--compilation-units`; the server always separates |
| `compile.include_dirs` | strings | both | `-I` |
| `compile.defines` | `NAME[=VALUE]` strings | both | `-D` |
| `compile.param_overrides` | table | both | `--param-override`/`-G`; string, integer, float or boolean values |
| `compile.system_tasks` | strings | llg | `--define-system-task` prototypes |
| `libraries.map_files` | strings | llg | `--libmap` |
| `libraries.files` | `[lib=]path` strings | llg | `--libfile`; only the path is resolved |
| `libraries.order` | strings | llg | `--library-order` |
| `libraries.default` | string | llg | `--default-library` |
| `lint.enabled`, `lint.rules.<id>` | | both | rule switches/severities (applied to every `llg` lint pass) |
| `lint.only` | bool | llg | `--lint-only` |
| `lint.warnings_as_errors` | bool | llg | `-Werror` |
| `lint.json` | bool | llg | `--lint-json` (stdout); implies `only` |
| `lint.json_file` | string | llg | `--lint-json <path>`; implies `json` |
| `analysis.max_file_bytes`, `analysis.max_total_input_bytes` | integers | ls | input budgets |
| `simulator.stop_policy` | `"resume"` \| `"exit"` | llg | `--stop-policy` |
| `simulator.optimize` | bool | llg | `--no-opt` is `optimize = false` |
| `simulator.plusargs` | strings | llg | arguments after `--` (they replace; `--append-plusarg` adds) |
| `waveform.file` | `.vcd`/`.fst` path | llg | `--wave` |
| `waveform.depth` | integer 0 to 4294967295 | llg | `--wave-depth`; requires `file` |
| `build.gen_only` | bool | llg | `--gen-only` |
| `build.generator`, `build.launcher`, `build.cc`, `build.cmake` | strings | llg | `--generator`, `--launcher`, `--cc`, `--cmake` |
| `build.cflags` | string | llg | `--cflags` (an empty value means no flags) |
| `build.model_opt_level` | `O0` `O1` `O2` `O3` `Os` | llg | `--model-opt-level` |
| `build.jobs` | positive integer | llg | `--build-jobs` |
| `build.dpi_libs` | strings | llg | `--dpi-lib` |
| `output.out_dir` | string | llg | `--out-dir` |
| `output.runtime_cache` | string | llg | `--runtime-cache` |

Command-line-only: `--config`, `--help`, `--version`, and the negations
`--no-gen-only`, `--opt`, `--no-lint-only`, `-Wno-error`, `--no-wave`.

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
warnings_as_errors = true

[simulator]
stop_policy = "exit"

[waveform]
file = "build/waves/tb.fst"

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

- `top` (string) — elaboration top module name (auto-detected when omitted).
  A `library.` prefix or `:config` suffix is rejected; a configuration is
  selected by its plain name when no module has that name.
- `edition` (`"v2001"` or `"sv2009"`, `llg` only) — language edition
  (`--edition`, default `sv2009`). `llg_ls` always compiles SystemVerilog-2009.
- `compilation_units` (`"separate"` or `"merged"`, `llg` only) — compilation-unit
  grouping (`--compilation-units`, default `separate`). `merged` preserves each
  source buffer's identity while sharing preprocessing and `$unit` scope;
  library sources group per library, in admission order, separately from work
  sources. `llg_ls` always uses `separate`.
- `include_dirs` (array of strings) — additional include-search directories
  (may be external); source directories are already include dirs. In `llg`
  they are also searched for module definitions: a module, interface, program
  or package that the sources use but do not define is loaded from the
  `.v`/`.sv` file directly inside one of these directories that declares it,
  and two files declaring the same needed name is an error.
- `defines` (array of `NAME` or `NAME=VALUE`) — preprocessor defines applied
  to every analyzed source (`-D`); they drive `` `ifdef ``/`` `elsif ``
  selection and macro expansion.
- `param_overrides` (table, `NAME = <string | integer | float | boolean>`) —
  top-level parameter overrides; they apply to the top-level instances only.
  `llg` accepts the same overrides with `--param-override NAME=VALUE` (`-G`).
  A TOML string is the `VALUE` text of `-G NAME=VALUE`; an integer is its
  decimal value, a float its exact real literal and a boolean `1'b1`/`1'b0`.
  The value is converted to the parameter's declared type as in an assignment:
  - integral parameters take sized/based literals, X/Z digits, negative values
    and assignment patterns; an unsized decimal of any size keeps its value (an
    untyped parameter gets the value's bits plus a sign bit);
  - a `string` parameter takes `VALUE` as its exact text, unless the whole
    `VALUE` is a double-quoted literal, whose escapes then apply
    (`-G MSG=hello` and `-G 'MSG="hello"'` are the same);
  - an enum parameter accepts a member name;
  - a type parameter takes a data type (`-G 'T=logic [7:0]'`), also when it has
    no default.

  Overriding a `localparam`, a name no top module declares, an unparsable value
  or one that does not convert to the type is an error naming the override.
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

Every `llg` run lints the elaborated design before generating the model:
lint errors stop the run (exit 1), warnings are printed and the run continues.

- `enabled` (bool) — global switch; `false` disables every rule (a per-rule
  entry can re-enable individual rules).
- `rules.<id>` (table per rule) — `enabled` (bool) and `severity`
  (`"error"` | `"warning"` | `"info"`). Unknown rule ids are errors. `llg`
  applies `enabled` and `rules` to every lint pass.
- `only` (bool, `llg` only) — lint and exit without generating the model
  (`--lint-only`).
- `warnings_as_errors` (bool, `llg` only) — report lint warnings as errors,
  which stop the run (`-Werror`).
- `json` (bool, `llg` only) — report lint as JSON on stdout and exit
  (`--lint-json`); implies `only`.
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
- `optimize` (bool) — simulator IR optimization passes; `false` is `--no-opt`.
- `plusargs` (array of strings) — arguments passed to the generated simulator
  (the arguments after `--`).

The frontend export of the elaborated design has no budget; a simulator
compile may use all available memory. `LLG_MEMORY_LIMIT_MB` remains the
optional process-wide guard.

## `[waveform]` — waveform dumping (`llg` only)

- `file` (string) — dump waveforms from time 0 into this `.vcd` or `.fst` file
  (`--wave`); the extension picks the format and a relative path resolves
  from the config directory. The design needs no `$dumpfile`/`$dumpvars`: the
  file replaces any `$dumpfile` name and the design's `$dumpvars` selections
  are ignored, while `$dumpon`, `$dumpoff`, `$dumpall`, `$dumpflush` and
  `$dumplimit` still apply. `LLG_SIM_WAVE_FILE` replaces the name when a
  built model is rerun.
- `depth` (integer, default 0) — hierarchy levels dumped below each top
  instance, as `$dumpvars(depth, top)`; 0 dumps every level (`--wave-depth`).

## `[build]` — model build (`llg` only)

- `gen_only` (bool) — emit the model sources without building (`--gen-only`).
- `generator`, `launcher`, `cc`, `cmake` (strings) — CMake generator, C compiler
  launcher, C compiler and CMake program. Programs are looked up by name, not
  resolved against the config directory.
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
