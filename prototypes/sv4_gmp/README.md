# Compact GMP-backed sv4 prototype

This is a **standalone C11 experiment**, not the production simulator backend.
`LLG_SV4_USE_GMP` selects the implementation for clients of this directory's
`include/sv4.h`. It does **not** switch models emitted by `llg`, and it is not a
Cargo feature or simulator command-line flag. Production `src/sim/rt/` remains
unchanged. Keep the default OFF until the migration plan's integration gates pass.

The frozen old value implementation is in `golden/`. Its eight files are copied
byte-for-byte from the supplied project and checked against `golden_manifest.json`.
`tools/verify_golden.py` also reports the current repository files as identical,
drifted or missing, with hashes. Production drift does not fail the immutable
oracle check and must never be copied silently into `golden/`. Use `--output`
for a JSON report and `--repository-root` when checking another checkout.
Both independent implementations can coexist in differential tests: the old
symbols are `sv4_*`, and the new symbols are `gmp4_*`. The selected facade maps
names at preprocessing time; it has no backend tag, function table or runtime
backend-selection branch. Ordinary function calls, representation checks and
GMP costs still exist. Unsupported operations do not fall back to the old engine.

## Build

A C11 compiler and CMake 3.16 or newer are required. Tests also use Python 3.8
or newer. GMP mode requires the **matching header and library of GMP 6.3.0**.
Use separate build directories so cached options cannot confuse comparisons.

From the repository root, set `P=prototypes/sv4_gmp`. The commands below are for a
Unix shell; native Windows dependency/toolchain qualification is still pending.

```sh
P=prototypes/sv4_gmp

# Legacy-only: neither discovers nor links GMP.
cmake -S "$P" -B build/sv4-legacy \
  -DLLG_SV4_USE_GMP=OFF -DLLG_SV4_BUILD_DIFFERENTIAL=OFF
cmake --build build/sv4-legacy --parallel 8
ctest --test-dir build/sv4-legacy --parallel 8 --output-on-failure

# GMP backend plus both backends in the independent test executables.
cmake -S "$P" -B build/sv4-gmp \
  -DLLG_SV4_USE_GMP=ON -DLLG_SV4_BUILD_DIFFERENTIAL=ON \
  -DGMP_ROOT=/absolute/path/to/gmp-install
cmake --build build/sv4-gmp --parallel 8
ctest --test-dir build/sv4-gmp --parallel 8 --output-on-failure
```

The same `tests/sample_model.c` builds in both configurations. To build a
GMP-only client without the golden library, set
`-DLLG_SV4_USE_GMP=ON -DLLG_SV4_BUILD_DIFFERENTIAL=OFF`. Differential mode
intentionally builds both libraries even when the selected client is legacy.
`BUILD_TESTING=OFF` disables CTest executables; the sample and selected-backend
benchmark remain available. Explicitly turn differential mode OFF as well to
exclude its comparison benchmark and unused backend.

An explicit `GMP_ROOT` never falls back to a system installation. Configuration
checks required public `mpn` APIs, version, supported limb configuration and,
when not cross-compiling, the linked version and limb size. A cross-build's
run probe still needs execution on the target. Do not mix headers, libraries,
architectures, compiler ABIs or backend descriptors.

### Build GMP 6.3.0 from source

GMP is **not** bundled into this prototype, downloaded automatically, or added
to Cargo. Supply an unpacked upstream GMP 6.3.0 source tree and build out of tree:

```sh
sh "$P/tools/build_gmp.sh" /absolute/path/to/gmp-6.3.0 \
  /absolute/path/to/gmp-build /absolute/path/to/gmp-install 8
# Use -DGMP_ROOT=/absolute/path/to/gmp-install in the configuration above.
# With a working multilib toolchain, prefix the helper command with ABI=32
# and use separate build/install directories for the 32-bit limb ABI.
```

The helper invokes upstream configure, make, make check and make install in a
separate build directory, producing a static library. It requires GMP's Unix
build prerequisites. It is **not a native MSVC/Windows ARM64 recipe**. This
experiment was run on Linux x86-64; other release targets require native evidence.
GMP's configured assembly/CPU choices belong to its build, not this selector.
Do not distribute a host-tuned dependency to incompatible target CPUs.
Preserve upstream license notices when bundling GMP in a release. This prototype
redistributes neither GMP binaries nor the user's IEEE PDFs. The Lapligence license is retained in `LICENSE`.

## Storage and ownership

`gmp4_t` has a union of two inline `uint64_t` words or two limb pointers, width,
and signedness. The descriptor measures 24 bytes on the recorded 64-bit test
host, compared with 32 bytes for the old type. Treat its payload and width as
private. Metadata is separate from any net or scheduler object.

| Shape | New storage | Old storage |
| --- | --- | --- |
| Width 0 | Empty; no payload | Empty; no payload |
| Width 1-64 | Inline A/B; no heap allocation | One allocation with three 64-bit words |
| Wide, no B storage | One exact-width A plane | Three exact-width planes |
| Wide, with B storage | Two exact-width planes in one allocation | Three exact-width planes |

Internal `(A,B)` codes are `0=(0,0)`, `1=(1,0)`, `X=(1,1)`, `Z=(0,1)`.
Public scalar state arguments retain the **legacy** codes `2=X, 3=Z`, not DPI's
scalar enumeration. Canonical old-plane conversion is:

```text
B = X | Z
A = (bits & ~B) | X
bits = A & ~B; X = A & B; Z = ~A & B
```

Wide values contain real `mp_limb_t` arrays, not aliased `uint64_t` arrays.
Nail-free 32- or 64-bit GMP limbs are accepted by the implementation; only the
64-bit limb configuration was run. The exclusive width limit remains `1 << 20`.
The public word adapter uses 64-bit old-format words regardless of GMP limb size.
Overlapping active X/Z import masks are rejected, and irrelevant input bits
under unknown masks are canonicalized. No raw-struct equality or serialization.

Initialize destinations with `SV4_EMPTY`. Value-returning operations produce
independent owners; by-value inputs are temporary borrows. Use `clone/copy` for
independent data and `move/replace` to transfer ownership. Destroy named owners
explicitly. Never destroy a copied wide borrow or retain it across a mutation of
its owner. There are no self-pointers into inline storage. Moving raw payload
storage does not permit relocating a scheduler-visible cell address.

```c
#include "sv4.h"

int main(void) {
    sv4_t a = sv4_from_u64(7, 1024, 0);
    sv4_t b = sv4_from_u64(9, 1024, 0);
    sv4_t result = SV4_EMPTY;
    sv4_replace(&result, sv4_add(a, b));
    int okay = sv4_to_u64(result) == 16;
    sv4_destroy(&result);
    sv4_destroy(&b);
    sv4_destroy(&a);
    return okay ? 0 : 1;
}
```

Link a client against CMake's `sv4_selected` interface target. It propagates
both the include path and the compile definition/link dependency. A manually
compiled client must receive the same selector as its library.

`gmp4_add_into` and `gmp4_mul_into` additionally support exact output/operand
aliasing and reusable output storage. Multiplication uses a caller-owned
`gmp4_workspace_t` initialized with `GMP4_WORKSPACE_EMPTY` and explicitly destroyed.
A workspace must not alias value payloads or be used concurrently. There is no
mutable global or thread-local scratch in the new backend. This is preparation
for independent worker ownership, not implemented parallel simulation.

Promotion allocates a B plane when X/Z is written. Copy/reuse may retain an
all-zero B plane to avoid churn. `gmp4_compact` explicitly removes it when safe.
`gmp4_bytes` includes retained payload, but excludes the descriptor, workspace,
allocator metadata and GMP's own temporary storage. Neither zero facade dispatch
nor a warmed application workspace implies that GMP uses no internal allocations.

## Scope of the implemented API

`api_coverage.json` retains the supplied name-only ledger: 37 mapped names out
of 110 candidates. The candidates include `sv4_t` from callback typedefs; the
frozen header actually declares 109 `sv4_*` functions plus three `llg_*` helpers.
The ledger omits macros and helper types. It is not a language-feature
conformance percentage. The complete facade audit remains follow-up work.
The common subset includes constructors, lifecycle, casts/resizing/two-state
conversion, add/subtract/multiply/negate, bitwise operations, equality, logical
operations and conditional merging. Additional prototype APIs provide state/word
inspection, bit mutation, compaction, reusable output/workspace and a restricted
wire resolver. See `include/gmp4.h` for exact signatures and contracts.

`include/sv4_cell.h` demonstrates default-X reg/logic-like cells, default-zero
and X/Z-coercing two-state cells, independent read snapshots, and equal-strength
wire/tri resolution. It is not a registered runtime variable or `llg_net_t`.
Widths must match for non-NULL wire drivers. Strengths, delays, charge, aliases,
force/release and event scheduling remain outside this demonstration.

Missing operations include division/remainder/power, shifts, reductions,
relational/wildcard comparisons, concatenation/streaming/selection plans,
real/string conversions, UDPs, general array conditionals and full net resolution.
Generated C packaging, cache/ABI selection, DPI/VPI, containers, scheduling and
waveforms have **not** been migrated. Existing production ABI 4 remains intact;
`GMP4_PROTOTYPE_ABI` is a separate experiment identifier, not a replacement ABI.

Semantic tests follow the supplied 2001/2009 standards. In particular, the
supplied 1800-2009 Table 11-20 gives X for the Z/Z arms of an ambiguous
conditional. Known-arm conditionals, copying and casts still preserve Z.
`resize` uses the requested sign for extension; `cast` uses the source sign.
GMP handles normalized fixed-width arithmetic, not HDL expression-sizing rules.

## Tests, benchmarks and inspection

```sh
# Independent sanitizer build; do not benchmark this configuration.
cmake -S "$P" -B build/sv4-asan -DCMAKE_C_COMPILER=clang \
  -DCMAKE_BUILD_TYPE=Debug -DLLG_SV4_USE_GMP=ON \
  -DLLG_SV4_BUILD_DIFFERENTIAL=ON -DLLG_SV4_SANITIZERS=ON \
  -DGMP_ROOT="$PWD/deps/gmp"
cmake --build build/sv4-asan --parallel 8
ASAN_OPTIONS=detect_leaks=1 ctest --test-dir build/sv4-asan --parallel 8 --output-on-failure

# Five-sample release measurements and complete raw timings.
python3 "$P/tools/run_benchmarks.py" build/sv4-gmp/sv4_benchmark benchmark.json
# Each configuration also has a benchmark linked only to its selected backend:
python3 "$P/tools/run_benchmarks.py" build/sv4-legacy/sv4_selected_benchmark legacy.json

# Unix GCC/Clang build and link rejection checks.
python3 "$P/tools/verify_build_switches.py" --compiler cc \
  --gmp-root "$PWD/deps/gmp" --output switches.json

# GCC/Clang + ELF objcopy/objdump: compare facade/direct machine code.
python3 "$P/tools/verify_dispatch.py" --compiler cc \
  --gmp-root "$PWD/deps/gmp" --output dispatch.json

# Recreate inventories or verify the oracle without compiling.
python3 "$P/tools/generate_api_coverage.py" "$P/api_coverage.json"
python3 "$P/tools/verify_golden.py"
```

CTest runs selected-client cells, golden integrity and verifier drift/failure checks, deterministic differential
checks, Python-integer expected arithmetic, ownership/boundaries, allocation
balance and intentional failures. Expected arithmetic vectors are generated at
build time, not stored as a giant fixture. Release checks remain active under
NDEBUG. ASan/UBSan cover the project C code; prebuilt GMP assembly is not
instrumented by those flags. Intentional-abort subprocesses disable leak scanning;
the normal allocation/lifetime tests do not.

The differential benchmark uses five warm-cache process-CPU-time samples, alternating backend
order and no LTO. Fresh-result and reusable-output cases are separate. Reuse
comparisons use the old fresh-result API because it has no corresponding reusable
operation. Memory figures are descriptor plus requested payload, not RSS, peak
memory or whole-simulator speedup. The selected-backend benchmark uses the common facade for fresh add/mul/AND,
clone/copy and X/Z AND without linking an unselected library. Its input sequence
is deterministic; it does not provide paired backend ratios or reuse cases.
Wider initial values and signal distribution
need real model workloads before default changes.

GMP does not guarantee SIMD for each operation. Its public `mpn` routines supply
optimized arithmetic and machine kernels; the measured AND kernel in the recorded
build was unrolled scalar code. The new X/Z logic operates on whole limbs.
Neither observation establishes explicit AVX/NEON acceleration. Qualification of
actual vector kernels and thresholds is deferred to the implementation plan.

The handoff's implementation plan, validation report, raw benchmarks and evidence
are outside the production source tree. Move dated local evidence to ignored
`persistence/` when continuing inside the repository; do not turn this README
into an audit journal.
