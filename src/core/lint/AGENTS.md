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
- `LintRegistry` — `default_rules()`, `all()`, `lint(ctx, config)`; the
  default rule set runs in a stable order, skipping disabled rules and
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
- `rules/` — one file per rule plus shared helpers:
  - `unused.rs` — `unused-signal`
  - `width.rs` — `width-mismatch`
  - `latch.rs` — `incomplete-case`
  - `combloop.rs` — `combinational-loop`
  - `multidriver.rs` — `multi-driver`
  - `casez.rs` — `casez-misuse`
  - `if_latch.rs` — `if-latch`
  - `style.rs` — `naming-style`
  - `implicit_net.rs` — `implicit-net`: flags nets Surelog auto-created from
    undeclared identifiers.  Signature in the owned db: a [`NodeKind::Net`]
    whose type info carries no typespec (kind `"other"`) — every declared
    net/var gets one; the auto-created `logic_net` does not, and its position
    is the creating use site.  Undeclared identifiers in other positions are
    not seen here: procedural LHS uses are Surelog elaboration errors
    ("Illegal lhs of type wire"), plain expression uses leave an unbound ref
    without a net object, and `` `default_nettype none`` makes Surelog report
    "Illegal implicit net" itself.
  - `case_default.rs` — `case-default-missing`: case/casex/casez without a
    default arm OUTSIDE the incomplete-case domain (which owns the exact
    case in combinational/latch processes): casex/casez anywhere, exact case
    in edge-sensitive/initial/final processes and in function/task bodies.
    Skips statements already in the incomplete-case domain so one location
    is never reported by both rules.
  - `comparison_width.rs` — `comparison-width-mismatch`: comparisons (`==`,
    `!=`, `<`, `<=`, `>`, `>=`, `===`, `!==`) whose operands both have known
    self-determined widths that differ; skips when either width is unknown.
  - `unconnected_port.rs` — `unconnected-port`: flags instance ports left
    unconnected — omitted from the connection list, positional gaps, and
    explicitly-empty `.p()` connections — uniformly at Warning.  The db
    captures per-port high-connection presence facts (`high_present`,
    `high_open`) and an owned `high_expr` tree because a resolved `high: None`
    alone is ambiguous:
    omitted ports have no `vpiHighConn`, `.p()` is a `vpiNullOp` operation
    marker (zero operands), and expression/constant connections
    (`.i(a & b)`, `.v(4'd0)`) are real objects that count as connected.
    `` `.* ``/`.name` shorthand resolve to ordinary refs.  Ports whose
    declaration carries a default value that the instantiation leaves
    omitted are NOT flagged: Surelog binds the default expression as the
    port's high connection.  Top instances and Surelog's SYNTHESIZED
    per-port copy interface instances (`analysis::iface_copy_instances` —
    the `low`-reachable clones plus their unwired same-(parent, name)
    twins) are skipped; findings sit at the instantiation site (the instance
    node), messages name the hierarchical display path, positions are
    clamped to the 1-based contract.
  - `mixed_assign.rs` — `mixed-assignments`: one Error per process whose
    statement body contains BOTH blocking (`=`) and non-blocking (`<=`)
    assignments ([`StmtKind::Assign`] only; proc-cont assign, force/release
    and declaration initializers are ignored), positioned at the process
    keyword.  Deliberate overlap with `blocking-in-always_ff` /
    `nba-in-always_comb` (kind-vs-block-type mismatches): no suppression,
    the diagnoses differ and this rule also covers block kinds the other
    two never check (plain level-sensitive always, initial/final).
  - `undriven.rs` — `undriven-signal`: flags a declared signal that is read
    but has no active procedural/continuous driver, declaration initializer,
    connected output/inout flow, or primitive output terminal. Top-level
    external inputs/inouts, explicitly open child ports, implicit nets,
    intrinsic pull/supply nets, and synthesized interface copies are skipped.
    Input/inout actual expressions are read through the owned port
    `high_expr`, while the historical direct `high` target remains available
    to model/codegen consumers.
  - `sensitivity.rs` — `incomplete-sensitivity-list`: flags a plain explicit
    level-sensitive `always @(...)` when a directly read input is absent.
    A local written earlier on every represented path is treated as a
    temporary; uncertain control flow and partial/selected writes fail safe
    by keeping the signal as an input dependency. Identical findings from
    cloned module instances are source-deduplicated.
    Edge, named-event, mixed/complex, implicit `@*`, special `always_*`, and
    nested timing-control forms are skipped when correctness cannot be proven.
  - `select_range.rs` — `out-of-range-select`: checks statically known bit,
    part, indexed-part, and unpacked-array selectors against owned elaborated
    bounds. Dynamic selectors, unresolved ranges, and ambiguous
    multidimensional packed shapes are intentionally quiet.
  - `xz_comparison.rs` — `xz-logical-equality`: flags `==`/`!=` with a direct
    X/Z/? literal operand (including transparent casts); case/wildcard
    equality, parameter references, and nonliteral expressions are skipped.
  - `duplicate_case.rs` — `duplicate-case-item`: flags every later exact
    captured literal repeated in one exact `case`. Wildcard cases and
    nonliteral/equivalent-but-differently-represented expressions are skipped.
  - `analysis.rs` — shared owned-db helpers: read/write collection,
    expression-width computation, scope/instance iteration, port-link
    bookkeeping (deterministic, deduped, first-encounter order),
    unconnected-port classification (`port_unconnected`), synthesized
    per-port interface-copy identification (`iface_copy_instances`).

## Requirements

- **No `unsafe`** — enforced by the repo grep rule
  (`grep -rn "unsafe" src --include=*.rs | grep -v src/ffi` must be empty).
- **No VPI access** — rules work on the owned `db`/`model` built by
  `core::db::Db::build`; `core::db` is the single VPI traversal point.
- **No LSP dependencies** (tower-lsp/gag/dashmap stay in `src/bin/llg_ls`).
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
