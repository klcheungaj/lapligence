# GTKWave libfst snapshot

Pristine upstream copies of the official GTKWave libfst writer sources shipped
with Verilator 5.032 (`/usr/share/verilator/include/gtkwave/`):

- `fstapi.c`, `fstapi.h`
- `fastlz.c`, `fastlz.h`
- `lz4.c`, `lz4.h`
- `fst_config.h`, `fst_win_unistd.h`, `wavealloca.h`

Do not edit these files. Lapligence's changes are the tracked patch in
[`patches/libfst`](../../patches/libfst), authenticated by exact clean and
applied digests. The root build script renders the patched copy into
`OUT_DIR/libfst`; `sim::rt::waveform_sources()` embeds that rendering and
writes it into waveform models only. A tree that is entirely clean or entirely
applied is accepted; a partial or mismatched tree fails the build.
`tests/runtime_value_storage/libfst.cmake` renders the same sources for the
standalone CMake probes.

The upstream copyright and license headers are preserved in every source (see
`THIRD_PARTY_NOTICES`). The libfst API and FastLZ portions are MIT licensed;
LZ4 carries its upstream BSD 2-Clause license.

Lapligence also selects zlib packing and disables libfst parallel mode, because
its own bounded SPSC worker owns all writer calls.
