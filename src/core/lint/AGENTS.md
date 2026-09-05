# core::lint — shared Verilog/SystemVerilog linter

## Purpose

A rule engine over the owned database + design model (`core::db::Db`,
`core::model::DesignModel`), shared by the LSP and the simulator:

- The LSP runs the lint pass inside `features::analyze` and merges findings
  into the published diagnostics with `source: "llg-lint"` (severity
  `Error` → `ERROR`, rule id as the diagnostic `code`).
- `llg --lint` prints findings (`file:line:col: [SEVERITY] rule:
  message`) and aborts with exit code 1 on lint errors before codegen.
  `--lint-config <path>` loads a `llg-lint.toml` (see below) that
  enables/disables rules and overrides severities for the pass.

Rules read only owned data — no VPI access, no raw FFI, no LSP dependencies.

## API

- `LintRule` — a stateless rule: `id()` (stable lowercase-hyphenated id),
  `description()`, `check(&LintCtx) -> Vec<LintDiag>`.
- `LintCtx<'a>` — `{ db: &'a Db, model: &'a DesignModel }`; all per-rule state
  lives here (e.g. a shared analysis cache can be added later).
- `LintDiag` — `rule`, `severity`, `file: Option<String>`, 1-based
  `line`/`col`, `message`.
- `LintSeverity` — `Error` / `Warning` / `Info`.
- `RuleConfig` — `{ enabled: bool, severity: Option<LintSeverity> }`; the
  default for an unconfigured rule is enabled with no severity override.
- `LintConfig` — per-rule `RuleConfig` keyed by rule id (private map, public
  accessors):
  - `new()` / `set(rule, cfg)` / `get(rule)` (default when absent) /
    `is_enabled(rule)` / `severity(rule)` (override, or `None`).
  - `parse_toml(text)` — hand-parses a `llg-lint.toml` (no serde/toml
    deps); returns `Err(Vec<String>)` with human-readable line-numbered
    messages, keeping the valid entries parsed so far (best-effort).
- `LintRegistry` — `default_rules()`, `all()`, `lint(ctx, config)`; the 24
  default rules run in a stable order, skipping disabled rules and
  applying severity overrides.
- `lint(db, model)` — convenience: run every default rule over a db + model
  with the default config.
- `lint_with_config(db, model, config)` — same, with a `LintConfig`.
- `diags_to_json(&[LintDiag]) -> String` — serialize findings as one JSON
  object (hand-built, no JSON dependency).  Schema:
  `{"diagnostics": [{"rule": "...", "severity": "error|warning|info",
  "file": "/abs/path.sv" | null, "line": 3, "col": 10, "message": "..."}],
  "summary": {"errors": 0, "warnings": 1, "infos": 0, "total": 1}}`.
  `line`/`col` are 1-based.  Strings are JSON-escaped: `"` → `\"`, `\` →
  `\\`, and control characters U+0000..U+001F use `\b`/`\f`/`\n`/`\r`/`\t`
  or `\u00xx`.

Rule-specific behavior and helper responsibilities live in
[rules/AGENTS.md](rules/AGENTS.md).

## Default registry

Stable order: `unused-signal`, `width-mismatch`, `incomplete-case`,
`combinational-loop`, `multi-driver`, `casez-misuse`, `if-latch`,
`naming-style`, `blocking-in-always_ff`, `nba-in-always_comb`,
`unused-parameter`, `implicit-net`, `case-default-missing`,
`comparison-width-mismatch`, `unconnected-port`, `mixed-assignments`,
`undriven-signal`, `incomplete-sensitivity-list`, `out-of-range-select`,
`xz-logical-equality`, `duplicate-case-item`, `empty-implicit-sensitivity`,
`assignment-in-condition`, `casex-statement`.

## Requirements

- **No `unsafe`** — enforced by the repo grep rule
  (`grep -rn "unsafe" src --include=*.rs | grep -v src/ffi` must be empty).
- **No VPI access** — rules work on the owned `db`/`model` built by
  `core::db::Db::build`; `core::db` is the single VPI traversal point.
- **No LSP dependencies** (tower-lsp/tokio/dashmap stay in `src/bin/llg_ls`).
- `unused-signal` still uses its historical activity model, but
  `undriven-signal` additionally accounts for captured structural primitive
  input/output terminals and expression-valued port actuals.

## Configuration (`llg-lint.toml`)

`LintConfig::parse_toml` reads a minimal TOML subset with no external deps.
Top-level `[rules.<id>]` sections configure one rule each; rule ids are the
stable lowercase-hyphenated ids (unknown ids are parse errors).  Missing
rules keep their defaults (enabled, the rule's own severity).

```toml
# llg-lint.toml
[rules.unused-signal]
enabled = false
severity = "warning"        # error | warning | info
[rules.width-mismatch]
severity = "info"
```

Blank lines, `#` comments (full-line and trailing after whitespace) and
leading/trailing whitespace are ignored.  `enabled = true|false` and
`severity = "error"|"warning"|"info"` are the only keys; anything else is
reported as a line-numbered parse error.  On any error the valid entries
parsed so far are still applied (best-effort).

Consumers: `llg --lint --lint-config <path> <file.sv>...` loads the
file (missing/malformed → error message + exit 1) and runs
`lint_with_config`; `llg --lint-json [<path>]` implies lint mode but is
report-only: it emits `diags_to_json` output on stdout (or writes it to
`<path>` — the token after the flag is the output path when it does not start
with `-`) instead of the human-readable lines, then exits without codegen or
simulation (exit 0 clean, 1 on lint errors); when both `--lint` and
`--lint-json` are given, `--lint-json` wins.  The LSP derives its
`LintConfig` from each root's `llg.toml` `[lint]` table (see
`src/bin/llg_ls/config.rs::translate_lint`); it does not use this file format.

## Interactions

- Below: `core::db` (`Db`), `core::model` (`DesignModel`), `core::elab`
  (value/width helpers used by `rules/analysis.rs`).
- Above: `src/bin/llg_ls/features.rs` (lint pass in `analyze`, merged into
  `llg-lint` diagnostics), `src/bin/llg.rs` (`--lint` gate before
  codegen).
