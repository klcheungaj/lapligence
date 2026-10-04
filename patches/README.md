# Vendored source patches

Root `build.rs` applies these patches with a portable Rust preflight, retaining
upstream-base vendor gitlinks. Clean and fully applied trees are accepted;
mixed/mismatched inputs fail, including edits outside diff hunks.

## Content and filesystem validation

Each patch directory has complete clean/applied `files.sha256` digests and a
required retired-target manifest. Slang's authenticated retired set includes the
old `SVInt.cpp` target. Missing, unexpectedly empty, extra or mismatched manifest
content fails with or without Git.

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
generated products stay outside vendors. These fixed inputs bound archive
admission.

## Slang

- Base: `7ddf4059f79eff508dd486eb42fd650cdf320d52`
- Source: `dc2161b6e7f87a8057e9498a7c983a2270d43c12`

| Patch | Purpose |
| --- | --- |
| `slang/slang-cache-only-source-reads.patch` | Cache-only admitted-buffer reads and lexical path normalization. |
| `slang/slang-ref-port-binding.patch` | Packed module-ref lvalue binding; subroutine refs retain separate rules. |
| `slang/slang-package-wildcard-export.patch` | Lazy finite wildcard re-exports, with ambiguous names diagnosed. |
| `slang/slang-conditional-z-merge.patch` | Selected packed conditional policy: definite true selects one arm despite other X/Z predicate bits; unpacked constants retain matching immediate members and default mismatches by member type, not initializer. Nested members default whole. Local edition policy, not an upstream erratum. Because patches cannot overlap, it also carries the operator-overload hooks in `OperatorExpressions.cpp` (below). |
| `slang/slang-output-port-runtime-select.patch` | Output ports connected to variables admit runtime selects as implied continuous assignments (SV 23.3.3.2); net and inout lvalues keep constant selects. |
| `slang/slang-operator-overload.patch` | Operator overload declarations (SV 11.11, A.2.8): `OverloadDeclaration` syntax in module, package, compilation-unit, generate and block items, an unnamed `OperatorOverload` symbol, and resolution only where a built-in unary, binary, increment, compound, assignment or cast operation is illegal. A resolved use becomes an ordinary `CallExpression` to the function found from the use's scope; increments and compound assignments become `A = f(A, ...)` assignments. |
| `slang/slang-net-alias-members-uwire-inout.patch` | Net aliases may name constant member selects of structure nets (SV 10.11, A.8.3/A.8.5). A uwire net may connect to a module inout port and that connection is not counted as a uwire driver (SV 6.6.2, 23.3.3.6–23.3.3.7); the simulator checks the collapsed net's drivers. Pass-switch terminals and `inout uwire` formals stay rejected. |

## Updating patches

Transition from the verified prior complete applied state or documented clean
base, then run the preparer. Never mix old applied files with new manifests,
relax full-file authentication, admit arbitrary intermediates, or discard unrelated
vendor edits to satisfy preflight.

## libfst

`vendor/libfst` is a plain directory of pristine upstream files tracked by this
repository (no submodule). The same preparer that patches Slang applies
`libfst/*.patch` in place there, with the same acceptance rules: a clean or a
fully applied tree, partial or mismatched content rejected with the offending
files named, nothing rewritten when already applied. Digests come from
`libfst/files.sha256`; `retired-files.sha256` is empty. The runtime
(`waveform_sources()`) and the standalone CMake probes read `vendor/libfst`
directly; the probes only verify the applied digests
(`tests/runtime_value_storage/libfst.cmake`) and ask for `cargo build` otherwise.
CI jobs without a Rust build (`dynamic-owners`, the `linux-test` archive runs)
apply the patch first with `git apply --whitespace=nowarn
--directory=vendor/libfst patches/libfst/libfst-local-changes.patch` from the
repository root, which yields the identical files.

Because the files are tracked, a built tree shows them as modified in
`git status`. **Never commit or stage the applied state.** The
`vendor_patches` test `committed_libfst_blobs_are_pristine` fails when HEAD or
the index holds anything but the pristine digests, so CI and any commit-time
test run catch it. Undo a build with
`git restore --staged --worktree -- vendor/libfst`; the next build re-applies
the patch. Local reproducible-run scripts accept the authenticated applied
state (`scripts/run-regression.sh`). A submodule would give a `dirty` marker
like Slang's instead of tracked modifications, at the cost of a separate
repository for nine files.

- Source: GTKWave libfst as shipped with Verilator 5.032 (`include/gtkwave/`)

| Patch | Purpose |
| --- | --- |
| `libfst/libfst-local-changes.patch` | Bounded writer buffering: the 128 MiB block and 2 GiB growth ceiling become 1 MiB with a 256 KiB increment. `_GNU_SOURCE` on non-Windows hosts so strict `-std=c11` builds declare the POSIX calls libfst uses. MSVC: `fst_config.h` leaves `HAVE_ALLOCA_H`/`HAVE_FSEEKO` undefined, `fstapi.c` takes its MinGW Win32 paths (`FST_WIN32_API`: file mapping, temporary files, `_fseeki64`, unbuffered reads), and the varint readers size their check buffers with enum constants instead of `const int` arrays (VLAs, rejected by MSVC). |
