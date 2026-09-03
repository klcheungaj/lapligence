# llg.toml — User Configuration

Every workspace root is an independent analysis root with its own effective
`llg.toml` (schema `1`), loaded from the root directory unless the client
overrides the path via `initializationOptions`
(`{ "llg": { "protocolVersion": 1, "configFiles": [{ "workspaceUri": …,
"path": … }] } }`). A missing file uses safe defaults (root as the sole
source directory, recursive `.v`/`.sv`, built-in excludes). The file is
watched and any parsed-config change hot-reloads the root — no server
restart required. Unknown fields or an unknown `schema_version` reject the
whole config atomically (a misspelled key cannot silently change analysis);
a malformed `defines`/`param_overrides` entry is dropped with a warning
instead. Relative paths resolve from the directory containing `llg.toml`.

## `schema_version`

Required integer; must be `1`.

## `[sources]` — source discovery

- `directories` (array of strings) — source directories; each is also an
  include-search directory. Default `["."]`; may point outside the workspace.
- `include` (array of globs) — root-relative include globs evaluated against
  each source directory. Only `.v`/`.sv` files become compilation units
  (`.vh`/`.svh` and other extensions enter analysis only through include
  resolution). Default `["**/*.v", "**/*.sv"]`.
- `exclude` (array of globs) — excludes win over includes. Built-in defaults:
  `slpp_all/**`, `.git/**`, `target/**`.

## `[compile]` — compilation inputs

- `top` (string) — elaboration top module (auto-detected when omitted).
- `include_dirs` (array of strings) — additional include-search directories
  (may be external); source directories are already include dirs.
- `defines` (array of `NAME` or `NAME=VALUE`) — preprocessor defines applied
  to every analyzed source (`-D`); they drive `` `ifdef ``/`` `elsif ``
  selection and macro expansion.
- `param_overrides` (table, `NAME = <string | integer>`) — top-level parameter
  overrides (`-PNAME=VALUE`, equivalent to `top -GNAME=value`); they apply to
  the top-level instances only, and an override no top module declares is
  reported as an error.

## `[lint]` — linter configuration

- `enabled` (bool) — global switch; `false` disables every rule (a per-rule
  entry can re-enable individual rules).
- `rules.<id>` (table per rule) — `enabled` (bool) and `severity`
  (`"error"` | `"warning"` | `"info"`). Unknown rule ids are errors.

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

## `[analysis]` — input-size safeguards

- `max_file_bytes` (positive integer) — maximum UTF-8 buffer or on-disk byte
  length of one unique compilation unit or resolved literal include. Default
  `1048576` (1 MiB).
- `max_total_input_bytes` (positive integer) — maximum sum of the measured
  unique compilation units and resolved literal includes in one analysis.
  Canonicalized paths are counted once, including include cycles. Default
  `8388608` (8 MiB).

Open UTF-8 buffer text is measured in preference to disk metadata. Discovery
is unchanged, so files over either limit remain watched; the root analysis is
rejected before staging or Surelog and publishes an `input-size-limit`
diagnostic while retaining the last-good snapshot. Every discovered/root
compilation unit must also produce a bounded UTF-8 snapshot; a missing or
unreadable root produces an `input-snapshot` diagnostic and is never passed to
Surelog on its live path. Open-buffer admission applies the same positive
per-file bound before storing or scheduling the buffer, using the built-in
default until a root-specific config is available. A rejected buffer is not
used by a later compile.

Literal includes are resolved beside the including file first, then in the
configured source directories followed by `compile.include_dirs`, in the same
order used for admission. A readable closed input is bounded-read once during
admission and that exact text is reused for isolation and shadow staging. LSP
compile options contain only the staged shadow include directories: this keeps
literal includes working while missing or macro-generated/dynamic includes
produce Surelog diagnostics instead of reading a live, unmeasured file. The
separate dump/general compile-options path retains live include directories
intentionally. Include or root staging failures are `input-staging`
diagnostics and reject the root, so a file changing after admission cannot
bypass either budget.

Config reloads are bounded independently of the analysis budgets: at most
`MAX_CONFIG_BYTES` (1 MiB) plus one byte is read from `llg.toml`. Oversized and
invalid-UTF-8 config files are reported as bounded `io::Error` load failures
and do not replace the last valid configuration.

## LSP diagnostic logging

Surelog invocation details are emitted only when LLG_LOG is set to debug or
trace, through the normal stderr or LLG_LOG_FILE logger; stdout remains clean
for JSON-RPC. Records include the accepted argv count, setter modes, a
per-argument representation bounded to 128 bytes, an overall representation
bounded to 2048 bytes, and a fixed-width fingerprint. Flag names and bounded
file/include paths remain visible for diagnosis. Values on -D and -P
arguments are replaced with <redacted>. A NUL-rejected argument is recorded
as a rejected invocation with argv_count=0 and is never included in an
accepted argv representation. Source contents are never logged.
