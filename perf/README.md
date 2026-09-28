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
the same method for both sides of an A/B comparison. The default C flags match
the generated model's Release optimization (`-O3`) and add the project's
warning flags. Use `--cflags '-O2 -Wall -Wno-unused-function'` for the separate
warning-clean build check required at `-O2`.

The corpus parameters are named tunables, not product thresholds. The task,
zero-delay, and wide-value standard sizes target seconds to tens of seconds on
the Phase 0 machine. The mandatory many-process shapes can take minutes on the
libaco baseline because they deliberately expose its process and NBA scaling.
Shared-machine results need at least three repetitions, medians, an `uptime`
record, and a later quiet-machine A/B run before they support a gate.

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
stacks. `backtrace()` can stop at a libaco stack boundary, so short stacks are
valid samples rather than errors; profiles may under-attribute callers above a
coroutine switch. This Linux/glibc tool avoids `perf`, but it is statistical and
`backtrace()` is not a substitute for platform unwind qualification.

## RSS attribution

Run a simulator under the Linux smaps sampler:

```sh
perf/scripts/rss_attribution.py --output /tmp/rss.tsv -- /path/to/generated/sim
```

The tool polls `VmRSS` and retains the `/proc/<pid>/smaps` snapshot nearest its
sampled peak. Classification is heuristic because malloc does not label libaco
allocations: `[heap]` is heap; executable or file-backed read-only mappings are
binary/text; large sparse private anonymous mappings and sparse mappings near
256 KiB or 1 MiB are libaco save stacks; a 3–64 MiB private anonymous mapping is
the shared-stack candidate; remaining anonymous writable mappings are heap.
Allocator coalescing helps identify thousands of save stacks as one large sparse
mapping, but unrelated sparse arenas can be false positives and value allocations
can be counted as heap/shared stack. The report retains mapping-level details so
ambiguous classifications can be reviewed rather than treated as exact accounting.

Tool logic has a dependency-free regression test:

```sh
python3 -m unittest discover -s perf/scripts -p 'test_*.py'
```
