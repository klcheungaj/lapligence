# Vendored source patches

These patches keep the superproject's vendor gitlinks at their upstream base
revisions while preserving the small Lapligence fixes needed by the native
build. The root `build.rs` applies them with a portable Rust preflight and
reports a mixed or mismatched checkout as an error. Each patch directory also
contains a `files.sha256` manifest with the complete clean and applied content
digests for every active patch target. This catches edits outside a unified
diff hunk even when the vendor tree is a source archive without Git metadata.
The Slang directory's `retired-files.sha256` records the base content of the
removed `SVInt.cpp` patch target so an old applied patch cannot survive a
patch-set update. Both patch directories carry a required retired-target
manifest; Slang's entry set is authenticated by the build helper and libaco's
manifest is explicitly empty. A missing, empty, extra, or digest-mismatched
manifest fails in Git and source-archive checkouts alike.

Manifest content digests use canonical LF line endings. Patch files and
manifests are tracked with LF by `.gitattributes`; a vendor source checkout may
use CRLF, which the preparer parses and preserves when rendering the patch
while hashing the canonical LF bytes. The rendered file must equal the
manifest's authenticated applied digest before any write is attempted.

Targets are checked with `symlink_metadata`, and every existing ancestor from
the project root is opened without following links; symlinks, Windows reparse
points, and outside repository resolutions are rejected. The preparer keeps
the repository and target parent attached to those capabilities through the
write. It uses the safe Rust `cap-std` and
`cap-fs-ext` handle-relative APIs for the staging file, permission update,
cleanup, and same-directory rename. On Linux, staging uses an anonymous
`O_TMPFILE` inode and publishes it with descriptor-relative `linkat`; Windows
uses an exclusive staging handle. Other targets retain the link-count and
identity checks. The preparer flushes the file and parent directory and rejects
changed capabilities or hard-link insertions before accepting a replacement.

When Git metadata and the Git executable are available, the preflight first
requires `git rev-parse --show-toplevel` to equal the canonical vendor
directory. It then checks the vendor `HEAD` against the pinned gitlink below,
rejects tracked changes outside active patch targets, and rejects untracked
source/build inputs. Git is optional: source archives, including an archive
nested inside an unrelated outer Git checkout, and copied vendor directories
use the exact content manifests and do not require `.git`, external Git, or a
`safe.directory` configuration. Slang's CMake target lists its source files
explicitly, and the runtime embeds only the fixed libaco `aco.h`, `aco.c`, and
`acosw.S` paths; generated build products are outside the vendor tree. Those
fixed inputs plus the portable manifests cover the files the patch preparer
can admit without VCS metadata.

## Slang

- Base: `7ddf4059f79eff508dd486eb42fd650cdf320d52`
- Source: `dc2161b6e7f87a8057e9498a7c983a2270d43c12`
- `slang/slang-cache-only-source-reads.patch`: cache-only source reads and
  path normalization used by the admitted in-memory frontend boundary.
- `slang/slang-ref-port-binding.patch`: packed lvalue binding for module
  reference ports; subroutine reference arguments retain their own rules.
- `slang/slang-package-wildcard-export.patch`: resolves lazy package wildcard
  re-exports for finite declarations and keeps ambiguous re-export names as
  frontend diagnostics.

- `slang/slang-conditional-z-merge.patch`: the selected published packed
  conditional policy. Logically true multi-bit predicates bypass ambiguous
  merging and evaluate only the chosen arm, even if other predicate bits are X/Z.
  This is local edition-policy work, not an upstream erratum.

When a tracked patch changes, do not mix old applied files with the new manifest.
Use a verified transition from the prior complete applied state, or the documented
clean vendor base, then run the existing preparer. Its exact full-file checks
must not be relaxed to admit arbitrary intermediate or locally edited files.
Never discard unrelated vendor edits to make a patch-state check pass.

## libaco

- Base: `d00631a9e143a8711c0a6e7b603a72b1e379b661`
- Source: `c45ffec47b9629247f5bb272947b711036e7fd8e`
- `libaco/libaco-asan-shared-stack.patch`: ASan shadow preservation for
  shared coroutine stacks and Linux `MAP_NORESERVE` stack mappings.

The source commits were audited as functional Phase 01 changes. No hunk was
identified as debug-only, so the complete deltas are retained in these patch
files.
