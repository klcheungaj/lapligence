# Integration-test support

Test-only harnesses have no production dependencies. `lsp.rs` frames stdio JSON-RPC
and enforces deadlines/child cleanup. `sim.rs` isolates CWDs, preserves complete
named frontend-negative diagnostics and bounds model execution. Serialize any
parent process-CWD changes and restore them on unwind.
