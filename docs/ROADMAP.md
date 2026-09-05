# Engineering Roadmap

This is the plan of record for repository-wide reliability and maintainability
work. Product feature status remains in `sim_features.md` and `config.md`.

## P1: Boundary correctness

- [x] Fail closed when the LSP cannot establish its private Surelog scratch CWD.
- [x] Support wide elaboration arithmetic without machine-word truncation.
- [x] Validate captured numeric payloads and bound synthesized value sizes.
- [x] Check first-party C runtime allocations and scheduler-time overflow.
- [x] Replace raw public VPI pointers with session-scoped opaque handles.
- [ ] Add a documented safety contract to every remaining FFI `unsafe` block.

## P2: Internal APIs

- [x] Share one captured-value decoder between elaboration and simulator bounds.
- [ ] Convert VPI integer discriminants to owned domain enums in `core::db`.
- [ ] Make DB/IR invariant-bearing fields private and add validated constructors.
- [ ] Replace public `Result<_, String>` APIs with module-specific error types.
- [ ] Add explicit DB and IR validation at processing-phase boundaries.

## P3: Cohesive modules

- [ ] Split LSP analysis, source-graph, fallback, symbol-index, and request logic.
- [ ] Split LSP scheduling/state, staging/security, diagnostics, and handlers.
- [ ] Split DB capture and simulator lowering/rendering by domain concept.
- [ ] Keep current entry modules as narrow facades during migration.
- [ ] Reduce executable `main` functions to argument handling and exit mapping.

## P4: Automation and maintenance

- [x] Run all targets/features, minimal-feature checks, Clippy, and tests in CI.
- [ ] Consolidate repeated simulator and LSP integration-test harnesses.
- [ ] Add sanitizer and dependency/security audit jobs.
- [x] Keep the Surelog gitlink on upstream and apply local patches only at build time.
- [ ] Track and maintain the platform and LSP safeguard documentation.

Each refactoring step must preserve the existing owned-data pipeline and pass
formatting, Clippy with warnings denied, no-default-feature library checking,
and the full test suite.
