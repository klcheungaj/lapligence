# Known issues

Open design limitations that are understood but deliberately deferred. Each
entry states the symptom, the cause, the intended direction and how to
reproduce it. Remove an entry when the fix lands.

## Unpacked net arrays are expanded one net per bit

**Status:** open; to be addressed with the type-system refactor.

### Symptom

An unpacked array of multi-driven nets, such as `wire [W-1:0] r[N]` with two
continuous drivers, a partial driver or a port connection, produces generated
C whose size, compile time and simulation work grow with `N × W`, even when
every bit of an element is connected identically.

Example: [`tests/fixtures/sim/continuation_20_23/continuous_contexts.sv`](../tests/fixtures/sim/continuation_20_23/continuous_contexts.sv)
is 88 lines, with four `continuous_case` instances at `W` = 1, 7, 65 and 129.
It generates a `model.c` of about 7.1 MB and 96,000 lines:

- 3,838 separate width-1 `llg_net_t` objects, each with its own driver,
  strength, index and scratch tables (387 per three-element array at
  `W = 129`);
- `llg_model_storage_defaults()` and `llg_model_storage_destroy()` of about
  16,500 lines each, one reset/destroy sequence per bit net;
- continuous-assignment processes of up to about 3,600 lines, one unrolled
  block per bit (clone, bit select, cast, `llg_net_write` to that bit's net).

The test that runs it (`sim_review_tasks20_23`
`continuous_arrays_keep_values_dependencies_and_static_pattern_topology`) is one
of the slowest in the suite because it compiles this model in both optimizer
modes.

### Cause

A net-array cell has one electrical group per bit
([`collection/net_arrays.rs`](../src/sim/codegen/lowering/collection/net_arrays.rs)
creates the `g_array_net_<array>_<element>_<bit>` groups;
[`collection/processes.rs`](../src/sim/codegen/lowering/collection/processes.rs)
contributes each bit through its own driver slot). Per-bit groups are the
simplest representation that is always correct: nets resolve per bit, and port
collapse, `alias`, partial drivers such as `window[3:2] = a[-1:0]`, and
per-bit force/release can join bits of different nets. The representation is
used unconditionally, however, so the common case of identically connected
bits pays the per-bit cost in storage, code and run time (one driver scan per
bit instead of word-parallel `sv4` resolution).

### Intended direction

1. **Range-partitioned electrical groups.** Split a net only at the boundaries
   where its connectivity changes (driver ranges, alias and port-collapse
   points, force targets, net-type changes) and give each maximal run of bits
   with identical connectivity one group of that width. Identically connected
   elements then resolve as one wide group each; the per-bit form remains the
   degenerate case, so semantics are unchanged.
2. **Loops over remaining bit groups.** Where bit-level groups are genuinely
   needed, emit a loop over a table of groups instead of one unrolled block per
   bit.
3. **Table-driven net storage (done).** Driver cells use one array per net;
   defaults, index resets, alias binding/cleanup and destruction use descriptor
   tables and loops. Items 1 and 2 remain open.

The net, strength, alias, port-collapse and force/release suites are the
oracles; a differential fixture across widths and connection patterns should
accompany the change.

### Reproduce

```sh
llg --gen-only --top tb --edition 2009 --out-dir <dir> \
    tests/fixtures/sim/continuation_20_23/continuous_contexts.sv
grep -c '^static llg_net_t g_array_net_' <dir>/sim/*/model.c
```

## High frontend memory use during Slang wrapper capture and import

**Status:** open; retained representations reduced, large typed-lowering peak remains.

### Symptom

Large elaborated designs still require substantially more generation memory
than their charged export data. Linux x86-64 release measurements on 2026-10-02–03
use `many_processes_registers_config`, two clock edges and three runs per point.
Before is `c260a74c` with diagnostic stage markers; after includes the retained-copy
fixes. GNU time peak RSS medians:

| Processes | Before peak RSS | After peak RSS |
| --- | --- | --- |
| 5,000 | 0.88 GiB | 0.53 GiB |
| 10,000 | 1.71 GiB | 1.04 GiB |
| 20,000 | 3.40 GiB | 2.07 GiB |
| 40,000 | 6.77 GiB | 4.12 GiB |

At 20k/40k the logical export remains about 306/611 MiB (320,406,270 /
640,766,270 charged bytes). Whole-process peak RSS falls about 39%; the peak
moves from rendering to typed lowering. At 40k sampled rendering RSS falls from
about 6.77 GiB to about 2.54 GiB. The earlier measurements of 3.28/6.54 GiB
were from a different run and remain historical context, not a stage profile.
These are generation-process peaks, not generated-simulator runtime memory.
Export size is approximately linear for this corpus; other shapes can differ.

### Cause

The [C++ wrapper](../src/wrapper/slang_c_api.cpp) materializes a complete
`LlgSlangSnapshot` while Slang's compilation is still live. Capture also uses
identity maps, pending-edge tables and separately stored strings. For this
corpus, each small register process contributes roughly 60 semantic nodes,
90 edges and 12 constants: about 16 KB of charged export data. Charged bytes
exclude container capacity, indexing overhead and Slang's own allocations.

The [safe FFI decoder](../src/ffi/slang.rs) copies native snapshot data into
owned Rust data before destroying the native owner. Dense semantic IDs now use
checked arena lookups, and decoded node/edge vectors reserve their validated
record counts exactly. Native strings share stable interned storage; logical
export charging still counts each occurrence. Finalizing ordered edges releases
their pending storage.

The [driver](../src/bin/llg.rs) releases the decoded snapshot after the single
owned DB import. Consuming generation releases the DB after typed lowering,
and collection state ends once the typed model owns its data. The
[renderer](../src/sim/emit_c/model.rs) borrows unchanged execution analysis and
retains one normalized body/key per exact sharing group. These changes remove
copies previously live throughout rendering. Borrowing library generation APIs
retain their caller's DB for reuse.

Typed lowering still overlaps the DB, semantic model, collection indexes and
typed IR. Native compilation and snapshot overlap during capture, and native
and Rust snapshots overlap during checked decoding. Owned strings, exact
constant payloads and source/identity records remain substantial. Sampled
stage RSS includes all live representations and allocator-retained pages;
it is not an exclusive allocation total for that stage.

`export byte limit exceeded` originates in llg's wrapper capture budget.
Raising that budget admits larger exports but does not reduce their memory
cost or bound total process RSS. A large design can still exhaust its export
budget, a record-count ceiling or available process memory.

### Intended direction

Reduce the remaining typed-lowering overlap and evaluate compact owned records,
shared exact constant/string payloads and chunked capture/import.
Preserve checked C ABI ownership and the single owned DB import; consumers
must not traverse native ASTs independently. Verify exact values, source
identity and diagnostics as well as generated-model behavior.

Use `--max-export-mib` to choose the simulator export budget and
`LLG_MEMORY_LIMIT_MB` for the optional process-wide memory guard. Budget for
the measured frontend peak, rather than the exported byte count.

### Reproduce

Run:

```sh
/usr/bin/time -v llg --gen-only --max-export-mib 4096 \
    --top many_processes_registers_config \
    --define LLG_CORPUS_N=40000 --define LLG_CORPUS_EDGES=2 \
    --out-dir <dir> perf/corpus/many_processes.sv
```

Read maximum resident set size from `time`. Use 5k/10k/20k/40k processes, at
least three runs per point, release binaries and medians. The Linux runner
`python3 perf/scripts/frontend_scale.py <release-binary> <evidence-dir>` records
GNU time, stage markers, sampled RSS and generated-C hashes, running points
serially. See [profiling](../perf/README.md#frontend-stage-scaling). The export
budget counts captured data, not the bytes of generated `model.c`.

## Frontend and C generation time grow superlinearly with design size

**Status:** open; dominant quadratic work isolated inside Slang driver analysis.

### Symptom

Doubling process count more than doubles Slang analysis work. The historical
release timings were 5.7/11.7/26.4/69.4 seconds at 5k/10k/20k/40k, with frontend
compile/copy growing 3.4× at the last doubling. The new release measurements on
2026-10-02–03 use three runs per point and the same two-edge corpus:

| Processes | Before wall s (range) | After wall s (range) | Before / after peak GiB |
| --- | --- | --- | --- |
| 5,000 | 12.73 (10.47–12.92) | 32.64 (30.60–43.71) | 0.879 / 0.527 |
| 10,000 | 20.41 (18.03–20.68) | 65.95 (64.18–80.25) | 1.708 / 1.041 |
| 20,000 | 39.08 (34.83–44.84) | 169.44 (95.77–196.61) | 3.397 / 2.066 |
| 40,000 | 79.69 (74.16–97.56) | 206.90 (182.91–431.97) | 6.773 / 4.120 |

| Stage (seconds) | 10k before / after | 20k before / after | 40k before / after |
| --- | --- | --- | --- |
| Frontend compile/copy | 4.42 / 18.20 | 9.89 / 35.76 | 26.74 / 140.74 |
| Slang driver/unused analysis | 1.17 / 7.79 | 4.46 / 22.89 | 17.83 / 128.12 |
| Wrapper capture/finalization | 1.37 / 4.89 | 2.23 / 9.05 | 3.89 / 5.54 |
| FFI decode/copy | 0.86 / 2.39 | 1.87 / 1.49 | 2.98 / 1.82 |
| Owned DB import | 3.13 / 7.32 | 6.02 / 21.15 | 10.59 / 11.12 |
| Typed lowering | 3.29 / 15.46 | 5.85 / 28.46 | 11.12 / 27.30 |
| C rendering | 4.95 / 14.46 | 9.35 / 36.06 | 19.11 / 35.16 |

Stage rows overlap and their medians must not be summed. Host contention varied
substantially between the baseline and final measurements; these wall times do
not establish a stable total speedup or a code-induced regression. A subsequent
unchanged-base 40k native-call control (stopped before DB import/rendering)
took 686.00 seconds, of which Slang analysis took 645.72 seconds. The baseline
analysis progression (about 1.17/4.46/17.83 seconds at 10k/20k/40k) and
source/CPU sampling isolate quadratic driver overlap work. Repeat timing on an
isolated host for a stable before/after speed comparison. The memory reduction
in the preceding entry persists across the measured points.

### Cause

Slang's `analysis::DriverTracker::addDriver` scans all prior overlapping drivers
before insertion into its interval map. This corpus updates shared `ones` and
`d_ones` counters from every ordinary `always` process. These procedural drivers
normally permit each other, but the overlap loop still visits their quadratic
number of pairs. A release 40k CPU sample attributed 42.2% to `addDriver` and
8.9% to the interval map's `overlap_iterator::treeFind`. Diagnostic elaboration
and wrapper capture scale much closer to the exported record count.

Project changes remove redundant dense-ID hashing and geometric growth of
fallibly decoded vectors, release intermediate owned representations, avoid an
unchanged execution-model clone, and stream exact body-sharing groups. Source
positions now index files once and use lazy UTF-8-aligned byte checkpoints
and a bounded offset cache. This removes repeated whole-prefix scans when both
source bytes and node count grow. Startup spawn filtering indexes final names
once instead of scanning them for each process. Work/capacity/retention
regressions cover these paths. The corpus has few finals; its measured quadratic
contributor is Slang's overlap analysis.
Large allocation volumes, generated text size and cache/allocator effects still
affect lowering and rendering; shared-host wall times vary substantially.

### Intended direction

An upstream Slang change is warranted: summarize or partition interval drivers
by categories that can conflict, so compatible ordinary procedural drivers do
not enumerate one another. Preserve mixed continuous/procedural, port,
single-driver procedure, initializer, uwire and user-defined net diagnostics,
including their ordering and source identity. This needs upstream analysis
regressions; no vendored patch is applied for this investigation.

Keep generated `model.c` byte-identical when changing generation algorithms.
The measured corpus and representative sharing/four-state/net fixtures match
the baseline, including both optimizer modes for the representative fixtures.

### Reproduce

Use the command in the frontend memory entry with `LLG_CORPUS_N` set to
5,000, 10,000, 20,000 and 40,000, and compare wall times.
