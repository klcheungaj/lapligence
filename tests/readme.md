# Integration tests

The integration suites cover the shared Surelog/UHDM pipeline, simulator
code generation and native execution, and the language server's framed stdio
contract. LSP fixtures live in `fixtures/lsp/`; their `test.json` manifests
use `llg.lsp.fixture/v1`, and HDL sources retain the declared fixture header.

Reusable process harnesses live in `support/`. `support/lsp.rs` owns LSP frame
parsing, JSON-RPC routing, request deadlines, graceful shutdown, and forced
child cleanup for both stdio acceptance suites. `support/sim.rs` owns
collision-resistant temporary directories, panic-safe CWD restoration, and
bounded generated-model execution.

Tests that invoke Surelog must use a fresh temporary working directory because
Surelog writes `slpp_all/`. Execution and elaboration success paths should use
`compile_checked`; focused error-reporting tests may use raw `compile` when
they need to inspect partial results. Specialized negative cases may retain
their own compile assertions while reusing the shared lifecycle helpers.
