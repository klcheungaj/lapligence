# sim/rt

Embedded C11 simulator and waveform runtime plus the libaco source snapshot used
when building generated models. These sources are compiled with each generated
model, not linked into Rust binaries.

`llg_value.h` defines the packed four-state data type and its public operations.
Each generated model supplies its own compile-time limb capacity: widths below
the backend's exclusive `1 << 20`-bit limit are represented with three
parallel `uint64_t` limb arrays, while the stored vector width is `uint32_t`.
`llg_value.c` implements arithmetic, comparisons, selects, formatting, equal-strength
wire/wired-AND/wired-OR resolution with bounded implicit pull/supply modes, and
packed/real/shortreal conversions, including wide division/modulo/power,
streaming permutations, inside-range matching, and wide
packed-to-real/real-to-packed conversion. It is a standalone C11 translation unit that
needs only the C and math libraries, with no scheduler or libaco dependency.
`llg_rt.h` includes the value header as a compatibility facade; `llg_rt.c` owns
process scheduling, signal writes, driver storage/resolved-value publication,
and simulation output.
`llg_string.h` / `llg_string.c` own byte-string allocation, copying, methods,
and packed/numeric conversions. Expressions consume owned strings; storage
reads clone them. Decimal real parsing and formatting retain that ownership
contract and are independent of scheduler state.
`llg_container.h` / `llg_container.c` independently own dynamic-array, queue,
and associative-array allocation and element/key operations. Packed elements
remain model-width `sv4_t` values rather than host-width integers; each
container has an explicit init/destroy lifecycle, checked allocation
arithmetic, and LRM-defined invalid-index/default behavior.
Driver cells default to Z; generated initialization seeds pending delayed
continuous drivers with X before process scheduling.
The optional `llg_wave.c` owns asynchronous waveform output.

`rt::value_sources()`, `rt::runtime_sources()`, `rt::string_sources()`, and
`rt::container_sources()` expose the four source pairs.
The shared source writer and CMake builder emit and compile them, including
for runtime self-tests and models with waveform output.

Keep vector semantics aligned with `core::elab`, check allocation/size/time
boundaries, and cover scheduler behavior through the standalone C self-tests and
Rust process-level integration tests.
