# Integration-test support

- Purpose: shared, test-only harnesses for integration suites.
- LSP: `lsp.rs` sends framed stdio JSON-RPC requests, enforces deadlines, and
  cleans up child processes.
- Simulator: `sim.rs` provides isolated temporary CWDs and bounded native-model
  execution.
- Scope: helpers have no production dependencies and remain below `tests/`.
- Execution: suites use isolated temporary CWDs and serialized execution where
  Surelog's process-global state requires it.
