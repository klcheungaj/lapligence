# Third-party notices

## Bundled sources

- `vendor/slang`: slang (MIT); licence in `vendor/slang/LICENSE`.
- `vendor/libfst`: GTKWave libfst, FastLZ and LZ4 (MIT, BSD-2-Clause); see
  `vendor/libfst/THIRD_PARTY_NOTICES`.
- `vendor/zlib`: zlib (zlib licence, `vendor/zlib/LICENSE`), compiled into waveform-enabled generated
  models.

## Bundled: GNU MP (GMP)

`vendor/gmp` is GMP 6.3.0. Generated models use GMP kernels for wide
multiplication, division, modulo, power and decimal conversion. `llg` writes
the generic-C subset of GMP those kernels need (`src/sim/rt/gmp.rs`) and the
tables generated for 64-bit limbs (`src/sim/rt/gmp/generated`) beside every
model, under `gmp/`, and compiles them into the generated runtime; `GMP_ROOT`
may name an installation instead. Lapligence executables carry that source
text but never link GMP.

GMP is copyright the Free Software Foundation, Inc., and is dual-licensed under
the GNU Lesser General Public License version 3 or later, or the GNU General
Public License version 2 or later (`LGPL-3.0-or-later OR GPL-2.0-or-later`).
Every generated runtime links GMP and is distributed under Lapligence's GPLv2,
so a distributed model uses GMP under its GPLv2 option: include the
GMP licence texts (`COPYINGv2`, `COPYINGv3`, `COPYING.LESSERv3`) and offer its
corresponding source. Generated projects carry both under `gmp/`. Details:
`src/sim/rt/value_gmp/readme.md`.
