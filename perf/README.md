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
