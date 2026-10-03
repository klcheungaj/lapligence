# Known issues

Open design limitations that are understood but deliberately deferred. Each
entry states the symptom, the cause, the intended direction and how to
reproduce it. Remove an entry when the fix lands.

## Declared net views retain per-bit binding metadata

**Status:** electrical groups and continuous writes are range-partitioned;
compact declared-view metadata remains open.

### Symptom

Wide unpacked net arrays still emit one alias-view binding row per declared bit,
so descriptor data grows with `N × W`. Identically connected bits now share wide
electrical groups and word-parallel resolution; contribution writes use contiguous
part selects and descriptor loops for remaining groups.

Example: [`continuous_contexts.sv`](../tests/fixtures/sim/continuation_20_23/continuous_contexts.sv)
is 88 lines, with four `continuous_case` instances at `W` = 1, 7, 65 and 129.
An earlier snapshot before table-driven storage and operand-traffic elision
emitted about 7.1 MB and 96,000 lines. A separate Linux release comparison at
base `c260a74c` and after range partitioning measured:

| Metric | Before | After |
| --- | ---: | ---: |
| `model.c` bytes | 5,638,045 | 1,008,174 |
| `model.c` lines | 67,415 | 12,259 |
| Array electrical groups | 3,838 | 76 |
| Generation seconds | 0.366 | 0.071 |
| Clean model build seconds | 18.089 | 3.063 |
| Build and execution seconds | 49.994 | 3.967 |

Current emission combines table-driven storage lifecycle work, eligible operand
borrows and matching-cast elision with range-partitioned groups and contribution
loops. Per-row contribution casts remain explicit because width/sign metadata
can differ between rows; declared-view binding metadata still grows with `N × W`.
The `sim_review_tasks20_23` test
`continuous_arrays_keep_values_dependencies_and_static_pattern_topology` compiles
this fixture in both optimizer modes.

Timings above are indicative, with eight build threads and a shared runtime cache.
The 64-element, 128-bit toggling-driver
[`runtime.sv`](../tests/fixtures/sim/net_partition/runtime.sv) witness emits 64
array groups instead of 8,192; its model shrinks from 9,188,411 to 807,091 bytes.

### Cause

[`collection/net_partition.rs`](../src/sim/codegen/lowering/collection/net_partition.rs)
combines adjacent canonical roots while member mappings, structural driver ranges,
force/release targets and effective type/delay owners agree. Declared net views
still use per-bit `IrNetAliasBinding` / runtime alias-part descriptors so waveform,
VPI and array dependencies retain their original shapes. Permuted mappings and
opaque projections conservatively retain bit groups. Short or disconnected
contribution gathers retain ordinary emission; homogeneous captured part-select
writes, including delayed writes, use descriptor loops.

### Intended direction

Represent declared alias views with range descriptors as well, preserving exact
bit correspondence, source names and observation/dependency behavior. This is
separate from electrical partitioning and table-driven net storage.

### Reproduce

```sh
llg --gen-only --top tb --edition 2009 --out-dir <dir> \
    tests/fixtures/sim/continuation_20_23/continuous_contexts.sv
wc -lc <dir>/sim/*/model.c
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

## Record member net aliases rejected by the frontend

**Status:** open; alias grammar and frontend admission are owned by RTL-011.

Slang rejects a member of a fixed record net as a non-net in an `alias` statement,
although ordinary member continuous drivers and inout record connections work.
For example, declare `typedef struct { logic [7:0] lane; } record_t;`,
`wire record_t values[1:0];` and `wire [3:0] mirror;`, then use
`alias values[0].lane[3:0] = mirror;`. Compilation reports that `lane` is not a
net before owned capture. Audit IEEE 1800-2009 §10.11 and the net-lvalue grammar
in §A.8.5, then apply a narrow tracked frontend patch for the legal forms; do
not bypass checked compilation or treat frontend rejection as an illegal-type
oracle. This admission gap prevents member alias identity tests from reaching
the otherwise shared electrical projection path.

## Remaining non-flattened fixed-value contexts

**Status:** open; RTL-002 and RTL-002b implement descriptor transport for
integral fixed arrays.

Integral variable arrays copy, compare, select rows, merge conditionals, stream
(including multiple segments and unaligned slices), initialize and pass through
input/output/inout/ref formals and returns of static, automatic and recursive
functions without becoming one packed value. Array-valued pattern items and
pattern-lvalue row scatter use the same views. Oversized unpacked records and
arrays of records still have no descriptor layout and retain the packed payload
limit. Descriptor pattern items and scatter targets whose rows are small dense
arrays inside an oversized source, `with` stream selections and nested oversized
streams reject with explicit diagnostics. Extending those paths through per-cell
source expansion would recreate the capacity cost.

Each call or view operand currently takes its own snapshot, so a whole-array
input argument or selected source is copied once more than necessary; the cost
is proportional to materialized cells, not to the logical extent. Stream and
ambiguous-conditional results are lazy images that keep their captured sources
alive; the first read of an untouched cell materializes it from those sources.

Explicit waveform registration materializes every registered cell, so dumping a
16M array has a proportional runtime memory/output cost despite bounded generated
source. Sparse cells also remain materialized after becoming default again, because
queued updates and selected observers require stable descriptor addresses.

Fixed-copy publication prepares every destination cell, range and change record
before the first value becomes visible, and runs observer callbacks only after
the complete image is committed. Callback-side allocation can still fail after
that commit; allocation failure is fatal, so no partial image is observed.


## Runtime-selected module reference connections have no qualified binding oracle

The fixed module-ref path requires constant actual selectors. A connection such
as `child c(values[index]);` rejects with an explicit runtime-reference-rewiring
diagnostic; it must not silently reread `index` on each access or invent a
canonical storage cell. IEEE 1800-2009 §23.3.3.2 describes hierarchical reference
binding, but the retained runtime-selector characterization has no adjudicated
binding/rebinding oracle. Qualify that boundary before enabling runtime-selected
connections. Static selected connections and nested packed projections execute.

## Native stack frames grow with a statement's format-argument count

**Status:** open; the host-stack bound is measured, not proven.

### Symptom

A `$display`-style statement with many arguments needs a native frame that grows
with its argument count. A 512-argument `$display` needs 159 KiB of stack at
clang `-O0`, so the 640 KiB `LLG_HOST_STACK_ESTIMATE_BYTES` is a measured bound
over the tested shapes rather than a proven one: a statement with enough
arguments, or several such statements on one call chain, can exceed it.

### Cause

The emitter declares each statement's format argument array
(`llg_fmt_arg_t _llg_format_args_N[argc]`) and its temporaries as automatic
variables of the generated function, so they live on the native stack for the
whole statement and their size is proportional to `argc`. Constant-size frames
and arena-backed recursion bound everything else; these arrays are the remaining
argument-count-dependent stack use.

### Intended direction

Move the argument arrays and their temporaries into heap-backed value scopes
(the same ownership used for other statement temporaries), so the native frame
stays constant-size regardless of `argc`. Then re-derive the host-stack estimate
from the constant frame sizes and keep `tests/generated_c_frame_lint.rs` covering
the new shape.

### Reproduce

Generate a design whose `initial` block calls `$display` with 512 integer
arguments (for example `$display("%0d ... %0d", a0, ..., a511);`), build the
model with clang at `-O0` (`--cc clang --cflags -O0`) and measure the frame size
of the generated process function, for instance with `-fstack-usage` or
`-Wframe-larger-than=`. The frame is about 159 KiB; halving `argc` roughly
halves it.

## Queue and dynamic-array `sort`/`rsort` are quadratic and re-evaluate `with` keys

**Status:** open.

### Symptom

Sorting 20,000 integers with `sort()` on a queue or dynamic array takes 9.3 s
at gcc `-O3`. A `with (expr)` key expression is evaluated again on every
comparison, so side effects and cost multiply with the comparison count.

### Cause

`llg_method_reorder` in [`container/methods.c`](../src/sim/rt/container/methods.c)
implements `sort` and `rsort` as insertion sorts that evaluate the `with` key
inside the comparison. That is O(n^2) comparisons and O(n^2) key evaluations.

### Intended direction

Use an O(n log n) sort (for example a stable merge sort) that evaluates each
element's key exactly once into a key array, then sorts element indices by those
keys, with bounded native stack depth and no per-comparison allocation. Keep the
existing ordering rules for ties and four-state keys, and re-check the key
evaluation order against the LRM.

### Reproduce

Fill a `int q[$]` or dynamic array with 20,000 pseudo-random values, call
`q.sort();` (and `q.sort with (item);`), and time the simulation with a model
built at gcc `-O3`. Doubling the size roughly quadruples the time.
