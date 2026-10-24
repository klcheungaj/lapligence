# Agent guide: src/ffi

The safe native boundary: the only module with Rust `unsafe`. It receives the
Slang capture stream, owns platform calls and exposes safe owned APIs.
Contracts: [readme](readme.md); the ABI itself: `src/wrapper/slang_c_api.h`.

## Directory map

- `slang.rs`: requests, `Limits`, `Snapshot`, error owners, `#[link]` metadata.
- `slang/stream.rs`: capture-stream receivers (module docs: catalogue, order,
  ownership, error flow).
- `slang/node_flags.rs`, `semantics.rs`, `sources.rs`, `tokens.rs`, `values.rs`,
  `diagnostics.rs`, `metadata.rs`: per-table decoding and validation.
- `platform.rs`, `secure_fs.rs`, `process_memory.rs`: platform layer
  ([cross-platform](../../docs/cross_platform.md#platform-layers)).

## Unsafe and ownership

- Each module denies `clippy::undocumented_unsafe_blocks` and
  `unsafe_op_in_unsafe_fn`; every unsafe operation states validity, lifetime,
  alignment, initialization and ownership.
- No panic crosses C: trampolines catch panics and abort the stream.
- Native Slang owners stay on the calling thread (no `Send`/`Sync`);
  `secure_fs`'s `DirectoryStream` is the one documented `Send` exception.
- Expose no native pointer or native-storage lifetime; records are borrowed only
  for their callback, so copy what is kept.

## ABI changes

- Mirror `slang_c_api.h` layouts with `#[repr(C)]` and keep tags, flags, record
  order and budgets aligned with the [wrapper](../wrapper/AGENTS.md).
- A new tag, flag bit or record bumps `LLG_SLANG_ABI_VERSION` on both sides
  together. Document codes, edge roles and auxiliary bits in the header, not here.
- Validate version, tags, known flags (reject reserved bits), reserved fields,
  pointer/length pairs, IDs, ranges and table windows. Malformed output is
  `InvalidNativeData`, distinct from valid unsupported HDL.
- Kinds, operations, subkinds, flags, roles and lexical categories are stable
  repository codes, never Slang enums or pointers.
- Add a `SemanticNode` boolean as a `SemanticFlags` bit with a named accessor
  (`slang/node_flags.rs`), never a `bool` field. Raise the
  `semantic_node_stays_compact` bound only with a measured reason.
- Keep `Limits` ceilings aligned with the wrapper's `kHardMax*` (`slang.rs` docs).
- Bound every count and byte total before allocation, on both ABI sides.
- Keep the static `#[link]` attributes for wrapper, Slang and fmt; the library and
  every final musl target must retain the native archives and wrapping.

## Platform and filesystem

- `secure_fs` admits paths through owned directory handles and identity checks;
  keep capability-relative containment and race/symlink/reparse/hardlink
  protections, never path-check-then-open.
- `process_memory.rs` holds OS calls only; policy stays in `memory_limit.rs`.
  Unsupported platforms return a typed error, never a zero footprint.
- Native input authentication and staging: [patch preparation](../../patches/README.md).

## Tests

- Prefer Rust-side FFI tests: layout/tag validation, ownership on every error
  path, budget boundaries, exact values and copy-before-release.
- Capture changes go through the wrapper and the [owned DB](../core/AGENTS.md);
  consumers never add native traversals.
