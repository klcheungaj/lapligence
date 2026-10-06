# Shared linter

The simulator and LSP share 24 default-enabled rules over owned `Db`/`DesignModel`,
without native AST or LSP dependencies. Stable rule IDs cover usage, widths,
drivers, control flow and assignment style. See the [registry](rules/mod.rs)
and [rule modules](rules/readme.md).

Every `llg` run lints before codegen: errors stop the run, warnings do not unless
`-Werror`. `--lint-only` stops after lint; `--lint-json [path]` reports JSON
without simulation. Rule settings come from the `--config` file's `[lint]` table:

```toml
schema_version = 1
[lint.rules.width-mismatch]
enabled = true
severity = "error"
```

LSP roots use the same `llg.toml` `[lint]` table, reload changes automatically and
publish findings as `llg-lint`. See [configuration](../../../docs/config.md).
