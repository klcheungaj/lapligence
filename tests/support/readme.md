# Integration-test support

This directory contains shared, test-only harness code. `lsp.rs` implements
the framed stdio JSON-RPC client used by the LSP acceptance tests, including
request timeouts and child-process cleanup. `sim.rs` provides temporary CWD
guards and bounded native-process execution for simulator and runtime tests.

The helpers have no production dependencies and intentionally live below
`tests/`, so acceptance mechanics do not become simulator or LSP runtime APIs.
