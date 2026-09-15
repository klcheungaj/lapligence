# ffi — Rust ↔ native boundary

## Purpose

Only this directory calls native APIs:

- `slang.rs` provides the safe, blocking Slang compilation API and converts the versioned C ABI
  snapshot into owned Rust data.
- `process_memory.rs` samples the process's physical footprint on Linux, macOS, and Windows for
  the shared memory-limit safeguard.

The native Slang implementation and public C declarations live under
[`../wrapper/`](../wrapper/AGENTS.md). Core, simulator, LSP, and binary code must use the safe
APIs from this directory and must not call the C ABI.

## Safety boundary

- This is the only source directory allowed to contain Rust `unsafe`. Enforcement: `grep -rn
  "unsafe" src --include=*.rs | grep -v src/ffi` must be empty.
- Shared layouts use `#[repr(C)]`; declarations must exactly mirror `src/wrapper/slang_c_api.h`.
  Treat any ABI version, enum tag, flag, reserved field, pointer, length, ID, range, or table
  window mismatch as invalid native data.
- Each module denies `clippy::undocumented_unsafe_blocks` and `unsafe_op_in_unsafe_fn`. Document
  the validity, lifetime, alignment, initialization, and ownership basis at every unsafe
  operation.
- Safe APIs never expose native pointers or a lifetime tied to native storage. Do not add `Send`
  or `Sync` implementations for native owners. The compile call and snapshot decoding remain on
  the calling thread.
- Fallible APIs use their module error types. Preserve native failure status and message where
  available; malformed native output is a separate `InvalidNativeData` failure.

## Slang ABI v3 contract

- `CompileRequest` borrows admitted source buffers and typed options for one blocking
  `llg_slang_compile` call. Input arrays and strings remain alive until it returns. Sources are
  explicitly compilation units or include-only buffers.
- The library-unit request flag keeps the same admitted buffers and limits but requests a
  declaration-only snapshot for bounded language-server recovery. Unknown request flags remain
  invalid ABI input.
- The native source manager performs cache-only reads with lexical path normalization. Include
  directories define lookup prefixes over admitted buffers; they do not authorize filesystem
  reads.
- Defines, top modules, include directories, parameter overrides, source bytes, diagnostics,
  value bits, output bytes, semantic records, edges, and lexical tokens are bounded before or
  during allocation on both sides of the ABI.
- An OK native status transfers one unique opaque snapshot owner. Non-OK status transfers an
  error owner and must not leak an unexpected snapshot. Both destroy functions accept null.
- HDL compilation errors are represented by a successful snapshot whose `has_errors()` flag is
  set. Argument, resource, frontend setup, exception, and bridge failures return `SlangError`.
- `llg_slang_snapshot_view` borrows arrays and strings from the snapshot. Rust validates every
  table and copies all records before the RAII owner calls `llg_slang_snapshot_destroy`. Error
  views follow the same copy-before-drop rule.
- Assertion sequence records carry checked repetition/range metadata and `SequenceConcat` edges
  carry checked cycle-delay ranges. Unbounded maxima use the ABI-owned `UINT32_MAX` sentinel and
  are converted to `Option<u32>` before leaving this FFI module; invalid ranges, flags, and
  repetition kinds are rejected as `InvalidNativeData`.
- Preserve the unconditional static link attributes for `llg_slang_wrapper`, `svlang`, and
  `fmt`; they carry the native archives through the Rust library target.

The owned snapshot contains admitted files; compiler and analysis diagnostics with related
locations; elaborated instances, parameters, types, and constant values; a flat semantic
node/edge graph; and lexical tokens linked to semantic records where Slang supplies a
relationship. Semantic kinds, operations, subkinds, flags, edge roles, and lexical categories
are repository-owned stable codes. Slang's C++ enum values and AST pointers never cross the ABI.

Source ranges use admitted file IDs and zero-based half-open byte offsets. Absent IDs use the
ABI's invalid-ID sentinel. Four-state integers use paired value/unknown limb arrays; real and
short-real retain distinct widths; SystemVerilog string constants remain byte vectors because
their content need not be UTF-8. All other ABI strings must decode as UTF-8.

## Process-memory contract

- Linux reads resident pages from `/proc/self/statm`; macOS uses `task_info`; Windows uses
  `GetProcessMemoryInfo`.
- Platform handles and returned structures are validated before conversion. Footprint
  multiplication saturates at `u64::MAX`; errors retain the platform source where possible.
- Unsupported platforms return a typed error. They must not silently report a zero footprint,
  because that would disable the shared safeguard.

For memory-limit policy and platform validation, read [`../AGENTS.md`](../AGENTS.md). For native
ownership and exception handling, read [`../wrapper/AGENTS.md`](../wrapper/AGENTS.md).
