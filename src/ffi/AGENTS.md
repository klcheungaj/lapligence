# Safe native boundary

Only `src/ffi/` may contain Rust `unsafe`. Keep platform/native primitives here,
expose stable safe ownership APIs, and explain every unsafe operation's soundness.
Each module denies `clippy::undocumented_unsafe_blocks` and
`unsafe_op_in_unsafe_fn`; explain validity, lifetime, alignment, initialization
and ownership at every unsafe operation. No panic may cross C. Native owners
remain on the calling thread; do not implement `Send` or `Sync`.

## Slang snapshot ABI

- Mirror `slang_c_api.h` layouts with `#[repr(C)]`. Validate ABI version, tags,
  known flags, reserved fields, pointer/length pairs, IDs, ranges and table windows.
  Use module error types, preserving native status/message; malformed output is
  `InvalidNativeData`, distinct from valid unsupported HDL.
- ABI v4 `CompileRequest` borrows sources/options until blocking
  `llg_slang_compile` returns, distinguishing units from include-only buffers.
  Library-unit recovery uses the same buffers/limits; reject unknown request flags.
  Cache keys are lexically normalized; include directories are lookup prefixes,
  not permission to read the filesystem.
- Bound defines, tops, includes, parameter overrides, source bytes, diagnostics,
  value bits, output bytes, semantic records/edges and tokens on both ABI sides
  before/during allocation. OK transfers one unique snapshot owner; non-OK an
  error owner. Destroy unexpected snapshots on failure; both destructors accept
  null. HDL errors set snapshot `has_errors`; argument/resource/setup/exception/
  bridge failures return `SlangError`. Validate and copy snapshot/error views
  before RAII destruction, including early returns. Expose no native pointer
  or native-storage lifetime.
- Preserve paired value/unknown limbs, distinct real/shortreal widths and
  arbitrary-byte SystemVerilog strings; every other ABI string is UTF-8.
  Source ranges use admitted file IDs and zero-based half-open byte offsets;
  absent IDs use the invalid-ID sentinel. Kinds/operations/subkinds/flags/roles/
  lexical categories are stable repository codes, never Slang enums or pointers.
- Check sequence repetition/ranges and `SequenceConcat` cycle ranges; convert
  unbounded `UINT32_MAX` maxima to `Option<u32>`, rejecting invalid ranges/flags/
  kinds. Validate optional member-default constant IDs; fixed-array defaults
  cross as exact flattened bitstreams. Retain declaration-order violations in
  compilation-unit lexical uses, including macro expansions.
- Keep static `#[link]` metadata for wrapper, Slang and fmt. The library and every
  final musl target must retain the required native archives/wrapping.
- Bound UDP semantic nodes/edges; reject an edge budget above 16 million. Each
  row owns exactly one valid window. Wildcard indexing has snapshot-wide ceilings
  of 65,536 assignments and 8 MiB key bytes; check expanded bucket/key costs before
  allocation.
- Lexical flag bit 3 (`LLG_SLANG_LEXICAL_DIRECTIVE`, owned `is_directive`)
  identifies directive text, including unexpanded macro bodies, independently of
  expanded/skipped state. Disabled conditional branch tokens also carry the skipped
  flag, even when they occur inside a directive. Edition checks must not treat
  inactive branches as source. Accept only the four defined bits and reject
  reserved bits; update both ABI sides together.
- `ConditionPattern` (`LLG_SLANG_EDGE_CONDITION_PATTERN`, role 38) pairs by index
  with `Condition`; `THEN`/`ELSE` are separate branch roles at index zero. Extending the
  semantic tag set does not change pointer/layout contracts. DB import validates
  these relationships; capture must not turn a pattern match into a Boolean test.

## Platform and filesystem calls

`process_memory.rs` reads Linux resident pages from `/proc/self/statm`, macOS
`task_info` and Windows `GetProcessMemoryInfo`. Validate handles/structures;
footprint multiplication saturates at `u64::MAX`. Preserve platform error sources.
Unsupported platforms return a typed error, never a misleading zero footprint.
Use checked/saturating conversions; shared policy remains in `memory_limit.rs`.

`secure_fs` admits paths through owned directory handles and identity checks.
Preserve capability-relative containment and race/symlink/reparse/hardlink
protections rather than reverting to path-check-then-open operations.
See [patch preparation](../../patches/README.md) for authenticated native inputs
and staging guarantees.

## Tests

Exercise layout/tag validation, ownership on every error path, budget boundaries,
exact values and copy-before-release. Prefer Rust-side FFI tests. Coordinate
capture changes with [wrapper](../wrapper/AGENTS.md) and
[owned DB](../core/AGENTS.md); consumers must not add independent native traversals.
