# FFI and platform boundary

This module contains the project's Rust unsafe/native operations and exposes safe
owned APIs to the rest of the library.

| Component | Responsibility |
| --- | --- |
| `slang.rs` and `slang/` | C ABI v4 requests, bounded snapshot/error owners, layout/tag validation, exact value/text copies and RAII destruction. |
| `process_memory.rs` | Platform process-memory counters and native resource limits. |
| `secure_fs` | Handle-relative filesystem admission and identity/race protection. |

Snapshot data includes source/lexical provenance, typed semantic edges, UDP tables,
sequence metadata and aggregate defaults. No native pointer or borrowed buffer
escapes the safe interface. See [wrapper](../wrapper/readme.md),
[owned database](../core/db/readme.md) and
[patch preparation](../../patches/README.md).
