# Integration tests

- Purpose: cover the shared Surelog/UHDM pipeline, simulator code generation,
  native execution, and the language server's framed stdio contract.
- Harnesses: shared test helpers are summarized in [support/readme.md](support/readme.md).
- LSP fixtures: `fixtures/lsp/` uses `llg.lsp.fixture/v1` manifests and declared
  `// llg-lsp-fixture:` source headers.
- Datatype fixtures: details are kept with [data_types](fixtures/sim/data_types/),
  [extended](fixtures/sim/data_types_extended/), [edges](fixtures/sim/data_type_edges/),
  [completion](fixtures/sim/data_types_completion/), and
  [next phase](fixtures/sim/data_types_next/).
