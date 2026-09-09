# Lint rules

- Purpose: individual checks over the owned `Db` and `DesignModel`.
- Components:
  - `mod.rs`: authoritative rule registry.
  - `analysis.rs`: shared graph and data-flow helpers.
  - Rule modules: policy-specific checks and diagnostics.
- Boundaries: use `LintCtx`; no native AST traversal or I/O.
- Related: [shared linter](../readme.md).
