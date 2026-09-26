# Integration-test support

Test-only harnesses have no production dependencies. `lsp.rs` frames stdio JSON-RPC
and enforces deadlines/child cleanup. `sim.rs` isolates CWDs, preserves complete
named frontend-negative diagnostics and bounds model execution. Serialize any
parent process-CWD changes and restore them on unwind.

`sim.rs` honors the optional `LLG_TEST_BUILD_DIR` environment variable for
temporary builds, including public CLI invocations. Relative paths resolve from
the Cargo workspace; unset uses the system temporary directory. Each invocation
creates a unique child directory and removes only that child on drop. See
[test storage configuration](../readme.md#test-build-storage) for tmpfs usage.
For concurrent worktrees, [the test runner](../readme.md#parallel-worktrees)
sets these overrides and `TMPDIR` with per-run isolation and a shared runtime cache.
