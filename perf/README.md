# Performance workloads and tools

`perf/designs/` contains the small legacy baseline suite.
`perf/corpus/` is the coroutine-migration corpus: self-terminating SystemVerilog
workloads with deterministic summaries. The corpus deliberately covers process
count, masked NBA, nested timing-task, zero-delay dispatch, and suspended wide
value/string costs rather than mirroring the conformance suite.

## Corpus runner

Run a quick correctness/measurement pass with:

```sh
perf/scripts/corpus.sh \
  --sim-bin target/release/llg \
  --size smoke --mode both --runs 1 \
  --scratch-dir /build/my-llg-corpus \
  --output-dir /path/to/results
```

The standard set includes the named design §13.1 shape
`many-masked-20k` (20,000 processes, 20 clock edges), the minimum 10,000-process
independent-register shape `many-registers-10k`, and the task, zero-delay, and
wide-value workloads. The larger `many-registers-20k` remains selectable. Use
`--list-configs` to inspect the catalog, repeat `--config NAME` for a subset,
or use `--many-size N` by itself to select both many-process variants at any
exact size from 10,000 through 1,000,000. Sizes are passed with `llg --define`;
the configuration tops apply them as module parameter overrides, so no source
edit is needed. Combine `--many-size N --config many-registers-N` (or
`many-masked-N`) to run only one custom variant.

For every configuration and optimizer mode, the runner generates and builds one
self-contained model, then executes it `--runs` times. `results.tsv` records
frontend/code-generation time, CMake build time, simulation wall time, peak RSS,
executable size, exit statuses, and the SHA-256 of simulator stdout;
`medians.tsv` summarizes each configuration. The phases use `perf_measure.c` and
Linux `wait4`. Generated projects go
under a unique directory below `--scratch-dir` (or `${TMPDIR:-/tmp}`), while TSV,
logs, toolchain metadata, and output hashes go under `--output-dir`. Use
`--keep-scratch` when the generated executable is needed for profiling. CMake
build time is intentionally a cold, self-contained generated-project build; use
the same method for both sides of an A/B comparison. The runner uses the
generated project's model optimization default (O3).
Use `--model-opt-level O1`, `O2` or `O3` to compare levels; `--cflags` supplies
extra flags after that level and can override it. Release contributes only
NDEBUG. For older binaries without the named option, pass the level through
`--cflags '-O2 -Wall -Wno-unused-function'`; the runner clears CMake's Release
optimization for them too. Use O2 for the separate warning-clean build check.

The corpus parameters are named tunables, not product thresholds. The task,
zero-delay, and wide-value standard sizes target seconds to tens of seconds on
the Phase 0 machine. The mandatory many-process shapes can take minutes on the
archived Phase 0 libaco baseline because they deliberately expose its process
and NBA scaling.
Shared-machine results need at least three repetitions, medians, an `uptime`
record. When choosing a level, retain each executable with `--keep-scratch`
and interleave at least five simulation repetitions per level (O1/O2/O3,
repeated) so all levels see similar host load. Report medians and spread,
alongside cold compile time. A later quiet-machine A/B run is required before
shared-host results support a gate.

## Compile-time harness

`perf/scripts/compile_time.py` measures how long generated models take to
build, for one or more `llg` binaries side by side:

```sh
perf/scripts/compile_time.py \
  --sim-bin base=/path/to/old/llg --sim-bin new=target/release/llg \
  --set ladder --scratch-dir /build/my-llg-compile \
  --output-dir /path/to/results
```

For every design and binary it records `--gen-only` time and peak RSS, then
configures the generated project with CMake (generated project defaults plus
optional `--cflags`, compile commands exported) and times
`cmake --build --parallel` (`--jobs`, default all cores). It then rebuilds each
model translation unit (`model*.c`) alone with its exact compile command, once
preprocessed for the expanded size and once with GCC `-ftime-report` or Clang
`-ftime-trace` for front-end, optimization and top-pass times. Sizes cover model
sources, objects and the executable; `--run` also runs each model once and
hashes its stdout. `--no-phases` skips the per-TU compiles.

Designs form a size ladder so super-linear growth is visible: `pca-512` through
`pca-4100` (`perf/corpus/pca_sites.sv`, one small process per procedural
continuous assignment site), `tasks-16/64/128` (`testbench_tasks.sv`) and
`many-registers-10k/20k/100k`. `--set smoke` (default) selects the two
smallest, `ladder` everything but the 100k design, `large` everything;
`--design NAME` picks individual designs and `--list-designs` prints them.
`results.tsv` holds one row per design, binary and repetition (`--repeat`),
`medians.tsv` the medians, `comparison.tsv` the key columns with build time
relative to the first binary, and `passes/` the top compiler passes per TU.
Compare binaries on an idle host; the harness runs them alternately per design
so both see similar load.

## SIGPROF sampling

Build the preload library, run a generated simulator directly, then symbolize:

```sh
perf/scripts/build_sigprof_sampler.sh --output /tmp/llg_sigprof.so
LLG_PROF_OUT=/tmp/model.raw LLG_PROF_HZ=99 \
  LD_PRELOAD=/tmp/llg_sigprof.so /path/to/generated/sim
perf/scripts/profile_symbolize.py /tmp/model.raw
```

`LLG_PROF_OUT` enables sampling; an unset variable makes the library inert.
`LLG_PROF_HZ` accepts 1–10,000 Hz and defaults to 99. The sampler warms the
unwinder before installing `SIGPROF`, writes fixed-size raw records with `write`,
and saves `/proc/self/maps` beside the raw file. `profile_symbolize.py` uses
those mappings plus `addr2line` to produce an inclusive flat TSV and folded
stacks. When profiling the archived Phase 0 baseline, `backtrace()` can stop at
a libaco stack boundary, so short stacks are valid samples rather than errors;
profiles may under-attribute callers above a coroutine switch. Current stackless
models have no such boundary. This Linux/glibc tool avoids `perf`, but it is
statistical and `backtrace()` is not a substitute for platform unwind
qualification.

## RSS attribution

### Process-count qualification (Linux)

`process_scale.py` uses one generated fork body and a `fork`/`join_none` loop,
avoiding one generated C body per instance. A synchronous DPI helper saves smaps
after all N children park on a named event. `perf_measure.c` supplies kernel
wait4 high-water RSS and elapsed time; steady RSS is the sum of that parked snapshot's
mapping RSS. Results include bytes/process (total RSS divided by N), attribution,
stdout hashes, commands, build failures/timeouts and medians with ranges. These
figures include the scheduler, groups, waits, libc and model storage.

```sh
python3 perf/scripts/process_scale.py \
  --binary current=/path/to/current/llg \
  --binary libaco=/path/to/llg-aco-f64c5de \
  --sizes 100000 300000 1000000 --repeat 7 --cpu 7 --jobs 10 --clocked \
  --output /path/to/new/evidence --work-dir /build/new-process-scale
```

Both directories must be new. The script warms each built model, then alternates
binary order for each measured pair on the selected CPU. `--clocked` also attempts
`many_processes.sv` registers at 100k/20 edges; it records peak RSS but has no parked
DPI snapshot. Generation/build default to 600-second limits and each execution to
3600 seconds; override with `--generation-timeout`, `--build-timeout` and
`--run-timeout`. A failed warm-up removes that binary from measured repetitions
and returns failure; missing measurements are not a passing gate. Fork-group
append and historical registry/wait paths can make this ladder quadratic even
though its generated spawner source stays small. Preserve evidence, then remove
only this run's work directory.

`coroutine_scale_{release,debug}` in the native CTest project separately asserts
zero warmed system chunk allocations and zero arena chunks in parked chains;
this does not claim zero allocations for scheduler waits or HDL values.

Run a simulator under the Linux smaps sampler:

```sh
perf/scripts/rss_attribution.py --output /tmp/rss.tsv -- /path/to/generated/sim
```

The tool polls `VmRSS` and retains the `/proc/<pid>/smaps` snapshot nearest its
sampled peak. It retains the Phase 0 categories so archived baseline measurements
remain comparable. Classification is heuristic because malloc does not label
libaco allocations: `[heap]` is heap; executable or file-backed read-only mappings
are binary/text; large sparse private anonymous mappings and sparse mappings near
256 KiB or 1 MiB are classified as historical libaco save stacks; a 3–64 MiB
private anonymous mapping is the historical shared-stack candidate; remaining
anonymous writable mappings are heap. Allocator coalescing helps identify
thousands of save stacks as one large sparse mapping, but unrelated sparse arenas
can be false positives and value allocations can be counted as heap/shared stack.
The report retains mapping-level details so ambiguous classifications can be
reviewed rather than treated as exact accounting.

Tool logic has a dependency-free regression test:

```sh
python3 -m unittest discover -s perf/scripts -p 'test_*.py'
```

## Coroutine tuning (Linux x86-64)

The four shipping tunables have different owners:

| Plan name | Implementation | Default |
| --- | --- | ---: |
| `LLG_CO_POLL_DEPTH_MAX` | `DEFAULT_POLL_DEPTH_MAX`, `ExecutionAnalysisOptions::poll_depth_max` in `src/sim/execution/analysis.rs` | 3 |
| `LLG_CO_EMBED_LIMIT` | `DEFAULT_EMBED_LIMIT`, `ExecutionAnalysisOptions::embed_limit` in the same file | 16 KiB |
| `LLG_CO_ARENA_MIN_CHUNK` | overridable macro in `src/sim/rt/llg_co.h` | 1 KiB |
| chunk-cache cap | `LLG_CO_CHUNK_CACHE_MAX_BYTES` in `src/sim/rt/llg_rt.h` | 1 MiB |

The first two plan names are conceptual names, not C preprocessor overrides.
The Rust analysis options are public library options; the CLI does not expose
these tuning knobs. MT dispatch threshold tuning belongs to the later MT work.

`designs/coroutine_tuning.c` and `scripts/tune_coroutines.py` sweep the production
coroutine macros, arena and runtime chunk cache without rebuilding Rust or adding
product knobs. Synthetic direct-call frame types are generated callee-first,
anchoring every limit+1 edges and restarting the poll budget below each anchor.
Oversized leaf frames use the real `LLG_CO_CALL_ARENA` path. This isolates the
mechanisms; it does not qualify Rust-emitted HDL semantics. The corpus and suite
remain independent gates.

```sh
python3 perf/scripts/tune_coroutines.py \
  --cpu 7 --jobs 24 --runs 7 \
  --work-dir /build/my-coroutine-tuning \
  --output /path/to/tuning-results
```

Both directories must be new. The build phase completes before any timed run;
executions run alone on the selected CPU. `--smoke --runs 1` executes eight
small configurations for harness checks. `--kind embed --value 131072` selects
just the 128 KiB embed candidate; other kinds and their listed values work likewise. Every execution checks its checksum,
coroutine status, LIFO payload preservation, empty retired arenas and cache cap
with checks active under NDEBUG. Allocation/check failures terminate the run.
The Python tool tests run with the other `perf/scripts/test_*.py` tests.

The full sweep has 274 configurations and seven passes with alternating
forward/reverse order, after one unmeasured warm-up per configuration:

- Poll limits 0/1/2/3/4/6/8, logical depths 1/2/3/4/8/16, 256 or 65,536 chains,
  and one or sixteen suspensions per completed leaf call. The large working set
  exceeds one core's L2; it is not a guaranteed cold-DRAM benchmark.
- Embed limits 0/4/16/64/128 KiB, payloads 256 B/4/16/64 KiB, 4,096 allocated roots,
  with all callees active or only one root in sixteen active. Payload size plus
  the frame header and counter determines the embedding decision. Roots touch
  their headers; active callees initialize and retain the full payload.
- Chunk sizes 256/512/1,024/4,096/16,384 B and cache caps
  0/64/256/1,024/4,096/16,384 KiB, each with one or 64 concurrent arenas and
  eight-deep tiny, mixed or large allocations. Each push writes its payload;
  every pop checks the first/last bytes and releases the arena when empty.

`results.tsv` retains each run's internal elapsed nanoseconds, operations,
wait4 wall time and peak RSS, exact root/live chunk bytes, retained cache bytes,
system allocations/frees and cache hits. `medians.tsv` reports median and
min/max ns per operation and median memory/counters. An operation is one chain
resume or one arena push/pop pair. Whole-process RSS includes startup/libc; exact
root and chunk accounting distinguishes retention from allocator rounding.
Native runs start fresh processes, so the first allocation round is included;
subsequent rounds exercise warmed caches. OS process exit reclaims the bounded
runtime cache after all arenas have been released.

For pinned corpus timing, use `corpus.sh --cpu N --runs 7`: builds stay unpinned,
each model has an unmeasured pinned warm-up, and only measured executions enter
`results.tsv`. `perf_baseline.sh --cpu N` likewise pins the run measurement child
while allowing model builds to use `CMAKE_BUILD_PARALLEL_LEVEL`. Its `cc` rows
sum compiler invocations; with parallel builds that sum is not model build wall
time. Use `compile_time.py` or the corpus build phase for the build-wall gate.

To separate compiler inlining from the poll/anchor mechanism and exceed the
shared last-level cache, repeat shortlisted poll limits with a million chains:

```sh
python3 perf/scripts/tune_coroutines.py \
  --kind poll --value 3 --large-chains 1000000 \
  --cflags '-fno-inline-functions -fno-inline-small-functions -fno-inline-functions-called-once' \
  --cpu 7 --jobs 24 --runs 7 \
  --work-dir /build/my-coroutine-poll-3 \
  --output /path/to/poll-3-results
```

Repeat with values 4, 6 and 8 in separate new directories. These flags leave the
header's forced-inline fast paths intact while preserving real C call costs
between the synthetic functions. The ordinary sweep allows the compiler to
inline direct callees, as production emission does. Both regimes matter when
choosing a generic default; a compact inlined benchmark alone cannot establish
the crossover for large generated task bodies.

`--caller-bytes 64 --large-chains 262144` adds a 64 B caller payload ahead of
every embedded child. This spaces resume headers across cache lines as frame
locals do, without adding payload operations to the call loop. It complements the
compact-header case; run the same shortlist with inlining disabled. A crossover
measured only on adjacent headers may not hold for frames containing live values.
### Measured tuning curves

Linux x86-64 Ryzen 9 7950X/WSL2, GCC 14.2, O3, CPU 7, seven serialized
repetitions on 2026-10-02. Times below are median [min,max] ns/operation.
The full per-shape curves and commands are retained in ignored
`persistence/stackless-coroutine/p7-perf/` in the main checkout.

Ordinary inlinable calls, 65,536 chains, sixteen suspensions/call:

| Poll limit | Depth 1 | Depth 3 | Depth 4 | Depth 8 | Depth 16 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 5.28 [5.00,5.63] | 7.04 [6.85,7.27] | 8.05 [7.62,8.23] | 13.90 [13.07,14.38] | 29.63 [26.39,31.75] |
| 1 | 4.10 [3.83,4.26] | 5.84 [5.38,5.95] | 6.73 [6.31,6.85] | 9.36 [8.81,9.55] | 16.95 [15.01,18.82] |
| 2 | 4.24 [3.95,4.27] | 5.60 [5.34,5.81] | 5.98 [5.73,6.23] | 7.68 [7.44,7.87] | 13.17 [12.34,14.26] |
| 3 | 4.15 [3.85,4.23] | 4.75 [4.48,4.89] | 5.87 [5.55,6.00] | 7.63 [7.22,7.87] | 12.96 [11.96,13.26] |
| 4 | 4.16 [3.90,4.35] | 4.77 [4.40,5.06] | 5.03 [4.62,5.23] | 7.34 [6.83,7.59] | 10.66 [10.06,12.06] |
| 6 | 4.12 [3.84,4.28] | 4.76 [4.39,4.97] | 5.03 [4.64,5.26] | 7.01 [6.72,7.23] | 9.82 [9.35,10.32] |
| 8 | 4.18 [3.90,4.53] | 4.70 [4.45,5.03] | 5.00 [4.72,5.31] | 6.52 [6.16,6.69] | 9.72 [9.35,10.74] |

Ordinary C-function inlining disabled, 1,000,000 chains, sixteen suspensions/call:

| Poll limit | Depth 3 | Depth 4 | Depth 8 | Depth 16 |
| ---: | ---: | ---: | ---: | ---: |
| 3 | 25.01 [22.82,25.99] | 27.99 [23.08,34.21] | 34.67 [33.32,93.82] | 51.78 [43.86,52.29] |
| 4 | 24.90 [24.73,25.97] | 26.42 [26.24,26.65] | 32.55 [32.16,33.08] | 45.80 [45.16,46.21] |
| 6 | 24.81 [24.49,25.57] | 26.38 [25.84,26.98] | 32.74 [31.98,33.14] | 43.43 [43.21,44.73] |
| 8 | 24.60 [24.50,25.51] | 26.03 [25.55,26.57] | 33.59 [33.22,33.89] | 41.77 [41.51,42.64] |

Separated headers: 64 B caller payload per level, 262,144 chains, ordinary
inlining disabled, sixteen suspensions/call:

| Poll limit | Depth 3 | Depth 4 | Depth 8 | Depth 16 |
| ---: | ---: | ---: | ---: | ---: |
| 3 | 34.37 [33.55,35.72] | 38.86 [38.42,39.85] | 53.75 [53.15,55.50] | 83.11 [80.50,84.44] |
| 4 | 34.50 [33.44,35.17] | 41.16 [40.62,43.23] | 82.26 [81.19,85.89] | 82.42 [80.53,83.27] |
| 6 | 35.13 [33.57,35.52] | 41.50 [39.76,42.31] | 57.64 [56.44,59.34] | 93.60 [90.10,98.34] |
| 8 | 34.97 [34.53,37.72] | 42.40 [40.95,43.11] | 66.78 [65.99,67.61] | 133.01 [132.21,137.82] |

Embedding: 4,096 roots, one suspension/call; frame payloads omit the additional
16 B header/counter. Sparse means only one root in sixteen calls the callee.

| Embed limit KiB | 4 KiB, all active | 4 KiB, sparse | 16 KiB, all active | 64 KiB, all active | Sparse 4 KiB root bytes / peak RSS KiB |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 126.80 [114.61,131.65] | 60.44 [57.90,63.21] | 1012.07 [963.64,1084.02] | 4277.79 [4009.75,4883.55] | 262,144 / 3,072 |
| 4 | 120.63 [112.61,131.57] | 59.88 [59.19,61.19] | 1026.16 [967.36,1113.47] | 4178.58 [4075.24,4271.04] | 262,144 / 3,072 |
| 16 | 116.11 [104.27,125.11] | 105.41 [97.29,109.65] | 1001.74 [963.86,1063.32] | 4082.56 [3972.27,4357.91] | 17,072,128 / 18,432 |
| 64 | 112.62 [102.99,123.47] | 103.29 [98.77,105.11] | 1016.61 [974.66,1078.23] | 4269.98 [4019.81,4509.70] | 17,072,128 / 18,432 |
| 128 | 118.93 [113.11,143.74] | 107.73 [104.53,112.61] | 1234.56 [999.95,1783.95] | 4945.27 [4663.04,5873.39] | 17,072,128 / 18,432 |

Arena/cache curves: 64 concurrent arenas, eight-deep allocation bursts. A mixed
burst is 64/256/1,024/4,096/16,384/1,024/256/64 B; the large burst alternates
16/64 KiB four times. Time includes payload writes/checks.

| Minimum chunk B (1 MiB cap) | Tiny | Mixed | Large | Mixed live chunk bytes |
| ---: | ---: | ---: | ---: | ---: |
| 256 | 3.94 [3.87,5.20] | 1296.95 [1282.92,1308.40] | 28986.78 [28098.56,30583.71] | 3,534,848 |
| 512 | 4.04 [3.95,4.11] | 1289.41 [1276.79,1296.97] | 28886.77 [27696.07,31044.34] | 3,516,416 |
| 1024 | 3.99 [3.91,4.03] | 1528.95 [1519.33,1716.30] | 29073.24 [28028.90,30365.74] | 3,614,720 |
| 4096 | 4.18 [4.09,4.40] | 1907.48 [1903.68,1924.70] | 29180.88 [28017.57,30864.26] | 3,940,352 |
| 16384 | 4.23 [4.18,4.30] | 1547.77 [1534.50,1564.25] | 29316.41 [27864.15,31171.40] | 3,149,824 |

| Cache cap KiB (1 KiB chunks) | Tiny | Mixed | Large | Mixed retained bytes |
| ---: | ---: | ---: | ---: | ---: |
| 0 | 5.86 [5.76,6.69] | 2205.56 [2185.22,2236.23] | 30621.19 [28825.66,31358.22] | 0 |
| 64 | 4.11 [3.98,4.15] | 2123.58 [2105.04,2149.82] | 29268.61 [26319.64,31044.94] | 64,800 |
| 256 | 4.00 [3.49,4.10] | 2035.50 [1678.24,2044.87] | 29897.59 [28361.80,31383.18] | 261,856 |
| 1024 | 4.00 [3.92,4.23] | 1543.48 [1518.14,1576.82] | 29922.21 [27586.01,30440.66] | 1,047,584 |
| 4096 | 4.00 [3.95,4.07] | 35.76 [34.78,37.02] | 26019.86 [24732.56,26846.04] | 3,614,720 |
| 16384 | 3.98 [3.92,4.11] | 34.88 [34.45,35.68] | 13748.79 [13281.12,14875.39] | 3,614,720 |

All four defaults are retained. Compact/inlinable frames favor larger poll
budgets, but with separated headers limit 4 regresses depth-8 long suspension
from 53.75 to 82.26 ns/resume (disjoint ranges); limit 6 regresses depth 16 from
83.11 to 93.60 ns. Thus a larger poll budget is not uniformly better. Embed
limits above 16 KiB do not improve all shapes: sparse 4 KiB callees prefer arena
storage (about 60 rather than 105 ns/resume and 3 rather than 18 MiB RSS), while
all-active small callees benefit from embedding. Lowering the chunk minimum
helps the mixed burst but is not a clear win across tiny/large and single-arena
curves. Larger cache caps accelerate sustained bursts by retaining more memory:
4 MiB retains 3,614,720 B in the mixed burst versus 1,047,584 B at the default;
16 MiB can retain up to 15 MiB more than the default. These are workload and
memory-budget trade-offs, not evidence for a universally better default.

### Frontend stage scaling

On Linux, build `llg` in release mode and run:

```sh
python3 perf/scripts/frontend_scale.py target/release/llg <evidence-dir>
```

The runner generates `many_processes_registers_config` with two edges at
5k/10k/20k/40k processes, serially, three times per point. It enables
`LLG_PROFILE_STAGES=1`, records GNU `time -v` output, samples process RSS every
5 ms, and records stage times and `model.c` SHA-256 hashes in `metrics.json`.
Use `--time <path>` when GNU time is installed elsewhere and `--sample-ms` to
change sampling. Do not run concurrent 40k points on a shared host. Short stages
can have no RSS sample; sampled RSS includes earlier retained data and allocator
pages. Frontend stages are `native` (Slang compilation and wrapper capture),
`ffi.stream` (native tables delivered to and decoded by the Rust receiver, with
the nested wrapper-side `wrapper.stream`) and `db.import`. Generation stages are
`semantic`, `lowering`, `execution`, `optimization` and `render`, whose nested
`render.artifacts`, `render.sharing`, `render.assemble` and
`render.identifiers` split C emission. Compare medians and retain individual runs because host contention affects
wall time. See [known issues](../docs/known_issues.md) for remaining limits.
