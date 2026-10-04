# FFI and platform boundary

This module contains the project's Rust unsafe/native operations and exposes safe
owned APIs to the rest of the library.

| Component | Responsibility |
| --- | --- |
| `slang.rs` and `slang/` | C ABI v10 requests, bounded snapshot/error owners, layout/tag validation, exact value/text copies and RAII destruction. |
| `process_memory.rs` | Platform process-memory counters and native resource limits. |
| `secure_fs` | Handle-relative filesystem admission and identity/race protection. |

Semantic node IDs are checked as contiguous arena indices before range-based
reference validation. Decoded node and edge arenas reserve the validated record
counts exactly; native ownership ends after all payloads have been copied.

Snapshot data includes source/lexical provenance, typed semantic edges, UDP tables,
sequence metadata and aggregate defaults. No native pointer or borrowed buffer
escapes the safe interface. See [wrapper](../wrapper/readme.md),
[owned database](../core/db/readme.md) and
[patch preparation](../../patches/README.md).

`Limits::default()` preserves the interactive/library capture budgets.
`Limits::simulator(bytes)` admits batch designs using larger native record
ceilings and the caller's explicit export byte budget. Export accounting covers
captured records and strings; it is separate from the process-memory guard.

The source map flag asks Slang to parse an admitted original map buffer with
its preprocessor. It requires compilation-unit admission and preserves macro
invocation locations in configuration diagnostics.
