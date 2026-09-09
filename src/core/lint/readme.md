# Shared linter

- Purpose: run the same owned-design checks in the simulator and language server.
- Inputs: `core::db::Db` and `core::model::DesignModel`; no native AST access or LSP dependencies.
- Rules: 24 enabled by default, with stable IDs and configurable severities.
  - Categories: signal usage, widths, drivers, control flow, and assignment style.
  - See the [registry](rules/mod.rs) and [rule modules](rules/readme.md).
- Simulator:
  - `llg --lint`: check before code generation.
  - `llg --lint-json [path]`: report without simulation.
  - `--lint-config`: load `llg-lint.toml` rule settings.
- Language server:
  - Configure each root through `llg.toml`'s `[lint]` table.
  - Findings use source `llg-lint`; configuration changes reload automatically.
- Configuration examples: [project configuration](../../../docs/config.md).
