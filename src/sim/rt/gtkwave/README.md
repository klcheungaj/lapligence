# GTKWave libfst snapshot

These files are copied from the official GTKWave libfst writer sources shipped
with Verilator 5.032 at `/usr/share/verilator/include/gtkwave/`:

- `fstapi.c`, `fstapi.h`
- `fastlz.c`, `fastlz.h`
- `lz4.c`, `lz4.h`
- `fst_config.h`, `fst_win_unistd.h`, `wavealloca.h`

The upstream copyright and license headers are preserved in every source. The
libfst API and FastLZ portions are MIT licensed; LZ4 carries its upstream BSD
2-Clause license. `fstapi.c` has one local buffering change: its 128 MiB
writer block and growth ceiling are reduced to 1 MiB (with a 256 KiB growth
increment), and the translation unit enables GNU/POSIX declarations on Unix
when the compiler has not already done so. Lapligence also selects zlib
packing and disables libfst parallel mode, because its own bounded SPSC worker
owns all writer calls.
