# Vendored source patches

Root `build.rs` applies these patches with a portable Rust preflight, retaining
upstream-base vendor gitlinks. Clean and fully applied trees are accepted;
mixed/mismatched inputs fail, including edits outside diff hunks.

## Content and filesystem validation

Each directory has complete clean/applied `files.sha256` digests and a required
retired-target manifest. Slang's authenticated retired set includes the old
`SVInt.cpp` target; libaco's is explicitly empty. Missing, unexpectedly empty,
extra or mismatched manifest content fails with or without Git.

Digests use canonical LF. `.gitattributes` tracks patches/manifests as LF;
rendering preserves vendor CRLF where present. Authenticate the rendered applied
digest before writing.

`symlink_metadata` plus link-free ancestor opens reject symlinks, Windows reparse
points and outside resolutions. Repository/parent capabilities remain attached
through staging, permission changes, cleanup and same-directory rename using safe
`cap-std`/`cap-fs-ext` APIs. Linux stages anonymous `O_TMPFILE` and publishes via
handle-relative `linkat`; Windows uses an exclusive handle; other targets retain
identity/link-count checks. Flush file and parent, rejecting changed capabilities
or inserted hardlinks before accepting replacement.

With usable Git metadata, require canonical vendor `rev-parse --show-toplevel`,
pinned HEAD, no out-of-set tracked changes and no untracked source/build inputs.
No-Git/copied archives, including those inside unrelated checkouts, use exact
manifests without Git or safe.directory setup. Slang lists sources explicitly;
runtime embeds only libaco `aco.h`, `aco.c`, `acosw.S`; generated products stay
outside vendors. These fixed inputs bound archive admission.

## Slang

- Base: `7ddf4059f79eff508dd486eb42fd650cdf320d52`
- Source: `dc2161b6e7f87a8057e9498a7c983a2270d43c12`

| Patch | Purpose |
| --- | --- |
| `slang/slang-cache-only-source-reads.patch` | Cache-only admitted-buffer reads and lexical path normalization. |
| `slang/slang-ref-port-binding.patch` | Packed module-ref lvalue binding; subroutine refs retain separate rules. |
| `slang/slang-package-wildcard-export.patch` | Lazy finite wildcard re-exports, with ambiguous names diagnosed. |
| `slang/slang-conditional-z-merge.patch` | Selected packed conditional policy: definite true selects one arm despite other X/Z predicate bits; unpacked constants retain matching immediate members and default mismatches by member type, not initializer. Nested members default whole. Local edition policy, not an upstream erratum. |

## libaco

- Base: `d00631a9e143a8711c0a6e7b603a72b1e379b661`
- Source: `c45ffec47b9629247f5bb272947b711036e7fd8e`
- `libaco/libaco-asan-shared-stack.patch`: shared-stack ASan shadow preservation
  and Linux `MAP_NORESERVE` mappings.

## Updating patches

Transition from the verified prior complete applied state or documented clean
base, then run the preparer. Never mix old applied files with new manifests,
relax full-file authentication, admit arbitrary intermediates, or discard unrelated
vendor edits to satisfy preflight.
