# Agent guide: src/core/lint

Shared linter for `llg` and `llg_ls`: stateless rules over the owned `Db` and
`DesignModel`. User contract: [readme](readme.md), [config](../../../docs/config.md).

## Core abstractions

All in `mod.rs` (item docs give the details):

- `LintRule`: `id()` (stable, lowercase-hyphenated), `description()`,
  `check(&LintCtx) -> Vec<LintDiag>`. Per-pass state lives in `LintCtx`.
- `LintDiag`: rule, severity, position and `logical`; rules leave `logical`
  `None`, the registry fills it from the Db source map.
- `LintConfig` / `RuleConfig`: enablement and severity overrides; `parse_toml`
  reads the standalone `llg-lint.toml` format.
- `LintRegistry`: `default_rules`, `all`, `lint`, `lint_with_config`.
- `diags_to_json`: the `--lint-json` object, hand-built without a JSON crate.

## Directory map

- `mod.rs`: API, registry, config parser, JSON writer.
- `rules/`: one module per rule plus `analysis.rs` ([guide](rules/AGENTS.md)).

## Rules

- No native traversal, raw FFI, I/O, `unsafe` or LSP dependencies.
- Keep policy shared between CLI and LSP; drivers never duplicate rule logic.
  `llg` reads `[lint]` from its `--config` file through
  `config::resolve::translate_lint`; LSP roots read their `llg.toml` `[lint]`.
- `parse_toml` stays a library API; no driver option reads `llg-lint.toml`.
- Rule IDs are stable: the CLI JSON and the LSP diagnostic code use them.
- Use `core::elab` for value/width facts.

## Pointers

- CLI output, exit codes and `--lint-json`: `src/bin/llg/main.rs` module docs.
- LSP publication (`source: "llg-lint"`, related `` `line`` origin):
  [llg_ls guide](../../bin/llg_ls/AGENTS.md).
- Tests for new rules: [LSP acceptance](../../../tests/readme.md#lsp-acceptance).
