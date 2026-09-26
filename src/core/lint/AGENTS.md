# Shared linter

Rules consume owned `Db`/`DesignModel` through `LintCtx`: no native traversal,
raw FFI, I/O, `unsafe` or LSP dependencies. The LSP runs lint in `analyze` and
publishes `source: "llg-lint"`, severity mappings and rule ID as diagnostic code.
`llg --lint` prints `file:line:col: [SEVERITY] rule: message` and exits 1 on errors
before codegen.

## API and registry

- `LintRule`: stateless `id()` (stable lowercase-hyphenated), `description()` and
  `check(&LintCtx) -> Vec<LintDiag>`. Per-pass state belongs to `LintCtx<'a>` with
  `db: &'a Db` and `model: &'a DesignModel`.
- `LintDiag`: rule, Error/Warning/Info severity, optional file, one-based line/col
  and message. `RuleConfig` defaults to enabled, with no severity override.
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

`llg --lint --lint-config <path>` rejects missing/malformed configuration with
exit 1 despite the parser's best-effort behavior. `--lint-json [<path>]` implies
report-only lint and wins over `--lint`: the next non-flag token is the output
path, otherwise output goes to stdout. Exit 0 when clean, 1 on lint errors;
never generate or simulate. LSP configuration instead comes from each root's
`llg.toml` `[lint]` via `config.rs::translate_lint`.

`unused-signal` retains its historical activity model; `undriven-signal` also
accounts for primitive terminals and expression-valued port actuals. Shared
helpers use `core::elab` for value/width facts. Keep policy shared between CLI
and LSP rather than duplicating rule logic in drivers.
