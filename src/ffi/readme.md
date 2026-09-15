# Rust FFI boundary

- Purpose: expose safe Rust APIs over the native Slang and platform C ABIs.
- Modules:
  - `slang.rs`: blocking in-memory Slang compilation and a bounded, owned
    snapshot of files, diagnostics, elaborated metadata, semantic graph records,
    and lexical tokens.
  - `process_memory.rs`: platform memory sampling for the process-memory guard.
- Ownership: inputs are borrowed only for the compile call. The C++ shim owns
  snapshot storage while Rust validates and copies it; RAII destroys the opaque
  owner afterward. No Slang AST address or native allocation crosses the safe
  API.
- Admission: compilation units and include-only buffers are supplied from
  bounded process memory. Include lookup remains cache-only and cannot read an
  unadmitted file.
- Representation: source ranges are zero-based half-open byte ranges. Stable
  repository codes describe semantic operations and edge roles; unsupported
  Slang constructs remain explicit records. Four-state values preserve value
  and unknown limbs, and SystemVerilog strings preserve arbitrary bytes.
- DPI metadata: imported subroutine C names and context/pure qualifiers are
  copied into owned records; no native syntax view is exposed to consumers.
- Errors: HDL errors remain diagnostics in a successful snapshot. Invalid
  input, configured-limit failures, frontend/bridge failures, and malformed ABI
  output return typed Rust errors.
- Safety: this is the only Rust directory permitted to contain `unsafe`; every
  exported API is safe and owns its returned data.
- Consumers: shared-core capture, simulator lowering, language-server features,
  and process safeguards.
- Related: [C wrapper](../wrapper/readme.md) and
  [shared core](../core/readme.md).

## Source organization

`slang.rs` retains raw ABI declarations, native link attributes, resource
ownership/cleanup and safe compile entry points. Its `slang/` children separate
snapshot capture, semantic records, tokens, diagnostics and value decoding.
They remain inside the same FFI safety boundary; consumers receive owned data.

See [the source map](../../docs/source_layout.md).
