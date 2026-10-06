# Shared linter

Rules consume owned `Db`/`DesignModel` through `LintCtx`: no native traversal,
raw FFI, I/O, `unsafe` or LSP dependencies. The LSP runs lint in `analyze` and
publishes `source: "llg-lint"`, severity mappings and rule ID as diagnostic code.
Every `llg` run prints `file:line:col: [SEVERITY] rule: message` and exits 1 on
errors before codegen (`-Werror` promotes warnings); a `` `line``-mapped position appends `` (`line file:line)`` after
`col`, and the LSP adds it as related information.

## API and registry

- `LintRule`: stateless `id()` (stable lowercase-hyphenated), `description()` and
  `check(&LintCtx) -> Vec<LintDiag>`. Per-pass state belongs to `LintCtx<'a>` with
  `db: &'a Db` and `model: &'a DesignModel`.
- `LintDiag`: rule, Error/Warning/Info severity, optional file, one-based line/col,
  message and `logical` (the `` `line``-mapped position). Rules leave `logical`
  `None`; the registry fills it from the Db source map. `RuleConfig` defaults to enabled, with no severity override.
- `LintConfig`: private rule map with `new`, `set`, `get`, `is_enabled` and
  `severity`; absent rules retain defaults. `parse_toml` uses no serde/TOML
  dependency and reports line-numbered `Err(Vec<String>)`, retaining valid
  entries for best-effort consumers.
- `LintRegistry::{default_rules, all, lint}` runs 24 rules in stable order,
  skips disabled rules and applies overrides. `lint(db, model)` uses defaults;
  `lint_with_config` accepts explicit policy. `rules/mod.rs::default_rules`
  owns registry order; [rule contracts](rules/AGENTS.md) owns behavior/helpers.

`diags_to_json` hand-builds one object without a JSON dependency:

```json
{"diagnostics":[{"rule":"id","severity":"error|warning|info","file":null,"line":3,"col":10,"message":"text"}],"summary":{"errors":0,"warnings":1,"infos":0,"total":1}}
```

`"logical_file"` and `"logical_line"` follow `"col"` only when a `` `line``
directive maps the position.

`file` is an absolute path or null; coordinates stay one-based. Escape quotes,
backslashes and U+0000..U+001F with JSON escapes (`\b`, `\f`, `\n`, `\r`, `\t`
or `\u00xx`).

## Configuration and drivers

The minimal `llg-lint.toml` format accepts `[rules.<id>]` sections with only
`enabled = true|false` and `severity = "error"|"warning"|"info"`. Unknown rules,
sections/keys or values are line-numbered errors. Ignore blank lines, surrounding
whitespace and `#` comments, including trailing comments after whitespace.

```toml
[rules.unused-signal]
enabled = false
severity = "warning"
[rules.width-mismatch]
severity = "info"
```

`parse_toml` (the standalone `llg-lint.toml` format) remains a library API; no
driver option reads it. `llg` takes rule settings from the `--config` file's
`[lint]` table (`config::resolve::translate_lint`), rejecting a missing or
malformed file with exit 1. `--lint-json [<path>]` implies `--lint-only`: the
next non-flag token is the output path, otherwise output goes to stdout. Exit 0
when clean or warnings only, 1 on lint errors; never generate or simulate. LSP
configuration comes from each root's `llg.toml` `[lint]` the same way.

`unused-signal` retains its historical activity model; `undriven-signal` also
accounts for primitive terminals and expression-valued port actuals. Shared
helpers use `core::elab` for value/width facts. Keep policy shared between CLI
and LSP rather than duplicating rule logic in drivers.
