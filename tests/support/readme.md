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

`sim_cli.rs` runs checked-in `.sv` stems or explicit `.v`/`.sv` filenames through
the public CLI in both HDL optimizer modes. `run_case_after_db_drop` supplements
that acceptance with checked compilation, validated whole-model generation after
snapshot destruction, and execution after Db destruction at native O0/O3. Its
expected output remains the caller's independent fixture oracle. Missing tools
fail mandatory acceptance; see [feature completion slices](../readme.md#feature-completion-slices).

`c_compiler.rs` identifies real GNU GCC (macOS `gcc` is Apple Clang) so GCC-only
diagnostics are requested only from GCC.
