# GTKWave libfst snapshot

Pristine upstream copies of the official GTKWave libfst writer sources shipped
with Verilator 5.032 (`/usr/share/verilator/include/gtkwave/`):

- `fstapi.c`, `fstapi.h`
- `fastlz.c`, `fastlz.h`
- `lz4.c`, `lz4.h`
- `fst_config.h`, `fst_win_unistd.h`, `wavealloca.h`

Do not edit these files and do not commit them patched. Lapligence's changes are
the tracked patch in [`patches/libfst`](../../patches/libfst), authenticated by
exact clean and applied digests. The root build script applies it in place, so
a built working tree shows these files as modified; `sim::rt::waveform_sources()`
embeds them and writes them into waveform models only. A tree that is entirely
clean or entirely applied is accepted; a partial or mismatched tree fails the
build. The standalone CMake probes (`tests/runtime_value_storage/libfst.cmake`)
verify the applied state. The `vendor_patches` test
`committed_libfst_blobs_are_pristine` rejects a HEAD or index that holds
patched content; restore with
`git restore --staged --worktree -- vendor/libfst` before committing.

The upstream copyright and license headers are preserved in every source (see
`THIRD_PARTY_NOTICES`). The libfst API and FastLZ portions are MIT licensed;
LZ4 carries its upstream BSD 2-Clause license.

Lapligence also selects zlib packing and disables libfst parallel mode, because
its own bounded SPSC worker owns all writer calls.
