# Vendored source patches

These patches keep the superproject's vendor gitlinks at their upstream base
revisions while preserving the small Lapligence fixes needed by the native
build. The root `build.rs` applies them with a portable Rust preflight and
reports a mixed or mismatched checkout as an error.

## Slang

- Base: `7ddf4059f79eff508dd486eb42fd650cdf320d52`
- Source: `dc2161b6e7f87a8057e9498a7c983a2270d43c12`
- `slang/slang-cache-only-source-reads.patch`: cache-only source reads and
  path normalization used by the admitted in-memory frontend boundary.
- `slang/slang-ref-port-binding.patch`: packed lvalue binding for module
  reference ports; subroutine reference arguments retain their own rules.

## libaco

- Base: `d00631a9e143a8711c0a6e7b603a72b1e379b661`
- Source: `c45ffec47b9629247f5bb272947b711036e7fd8e`
- `libaco/libaco-asan-shared-stack.patch`: ASan shadow preservation for
  shared coroutine stacks and Linux `MAP_NORESERVE` stack mappings.

The source commits were audited as functional Phase 01 changes. No hunk was
identified as debug-only, so the complete deltas are retained in these patch
files.
