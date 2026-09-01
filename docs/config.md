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

[lint]
enabled = true

[lint.rules.unused-signal]
enabled = false

[lint.rules.width-mismatch]
severity = "error"
```
