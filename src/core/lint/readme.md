# Shared linter

The simulator and LSP share 24 default-enabled rules over owned `Db`/`DesignModel`,
without native AST or LSP dependencies. Stable rule IDs cover usage, widths,
drivers, control flow and assignment style. See the [registry](rules/mod.rs)
and [rule modules](rules/readme.md).

`llg --lint` checks before codegen; `--lint-json [path]` reports without simulation.
`--lint-config` loads rule settings such as:

```toml
[rules.width-mismatch]
enabled = true
severity = "error"
```

LSP roots instead use `llg.toml`'s `[lint]`, reload changes automatically and publish
findings as `llg-lint`. See [configuration](../../../docs/config.md).
