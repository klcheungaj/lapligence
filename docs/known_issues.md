# Known issues

Open design limitations that are understood but deliberately deferred. Each
entry states the symptom, the cause, the intended direction and how to
reproduce it. Remove an entry when the fix lands.

## Declared net views retain per-bit lowering bindings

**Status:** electrical groups, continuous writes and generated declared-view
descriptors are range-based, and undriven net-array cells skip every per-bit
structure (KI-NET-INTERVAL); for connected bits the lowering-side alias graph
and binding list are still per bit. RTL-011 judged the full fix out of its
scope (see Cause).

### Symptom

Generated models now describe each declared net view with run-length
`llg_net_alias_part_t` rows: consecutive view bits that map onto consecutive
bits of one group driver slot share a `{ net, slot, signal_lsb, group_lsb,
bit_count }` row, and the runtime refresh/write paths move whole part selects.
The lowering still builds one `IrNetAliasBinding` per declared bit before the
emitter compresses them, so compiler memory and partition work grow with
`N × W` for wide unpacked net arrays and whole net-array inout ports.

Measured generated `model.c` bytes on Linux at the RTL-009 revision, with the
per-bit row equivalent computed from the same model:

| Fixture | Per-bit rows | Run-length rows |
| --- | ---: | ---: |
| [`continuous_contexts.sv`](../tests/fixtures/sim/continuation_20_23/continuous_contexts.sv) | 990,317 | 719,643 |
| [`runtime.sv`](../tests/fixtures/sim/net_partition/runtime.sv) (16,384 bits, 128 rows) | 823,656 | 236,756 |
| 64 × 128-bit whole net-array inout between two instances | 1,608,334 | 335,314 |

Earlier range partitioning (base `c260a74c`) had reduced `continuous_contexts.sv`
from 5,638,045 to 1,008,174 bytes and its array electrical groups from 3,838 to
76. The `sim_review_tasks20_23` test
`continuous_arrays_keep_values_dependencies_and_static_pattern_topology` compiles
that fixture in both optimizer modes.

### Cause

[`collection/net_partition.rs`](../src/sim/codegen/lowering/collection/net_partition.rs)
combines adjacent canonical roots while member mappings, structural driver ranges,
force/release targets and effective type/delay owners agree. It and the alias
union-find operate on `AliasBit` values, so declared views reach the emitter as
per-bit `IrNetAliasBinding` lists that waveform, VPI, force and array-dependency
lowering also consume bit by bit. Permuted mappings and opaque projections
conservatively retain bit groups.

The per-bit state is, in order: the alias union-find in
[`nets.rs`](../src/sim/codegen/lowering/collection/nets.rs) (`AliasBit` parent,
rank and membership maps for every bit of every alias-, selected-inout- or
net-array-connected net), the net-type collapse points in
[`net_collapse.rs`](../src/sim/codegen/lowering/collection/net_collapse.rs) for
the same bits, the partitioner's per-bit connection map, and finally the
`IrNetAliasBinding` list. Only the last is visible outside lowering, and it is
the smallest: making it run-length alone would not change the `N × W` growth.
A real fix replaces the bit union-find with an interval union-find over
`(owner, element, bit range)` and has the type plan, partitioner, net-array
publication, alias driver mapping, force targets and VPI/waveform views
consume ranges; every one of these is shared with ordinary net lowering, so
it needs its own task and full-suite qualification. Work stays linear in the
number of bits: RTL-011's same-depth collapse batches are linear in edges.

### Intended direction

Carry range bindings through partitioning and the IR so lowering work is
proportional to connected ranges, preserving exact bit correspondence, source
names and observation/dependency behavior. The undriven-cell classification in
[`collection/net_cells.rs`](../src/sim/codegen/lowering/collection/net_cells.rs)
already works on cells and shape classes; the remaining steps are, in order:
(1) an interval union-find over `(owner, element range, bit range)` replacing
the `AliasBit` maps for connected cells, split only where member mappings,
driver ranges, force targets or types differ; (2) a type plan and partitioner
over those intervals; (3) `IrNetAliasBinding` runs and a net-array publication
table that names a cell range per electrical group, with an emitter loop over
it; (4) range-aware alias driver mapping, `%v`, force, VPI and waveform views.

### Reproduce

```sh
llg --gen-only --top tb --edition sv2009 --out-dir <dir> \
    tests/fixtures/sim/continuation_20_23/continuous_contexts.sv
wc -lc <dir>/sim/*/model.c
```

## High frontend memory use during Slang wrapper capture and import

**Status:** open, narrowed; capture streams into Rust. DB import and typed
lowering set the generation peak for this corpus again since C rendering was
reduced (see
[High generation memory use during C emission](#high-generation-memory-use-during-c-emission)).

### Symptom

Large elaborated designs still require more generation memory than their
charged export data. Linux x86-64 release measurements on 2026-10-04 use
`many_processes_registers_config`, two clock edges and three interleaved runs
per point. Before is `b75dfc3f` (whole native snapshot, then decode); after
streams the capture into the receiver (`cb15fb64`). GNU time peak RSS
medians:

| Processes | Before peak RSS | After peak RSS |
| --- | --- | --- |
| 5,000 | 0.228 GiB | 0.208 GiB |
| 10,000 | 0.416 GiB | 0.399 GiB |
| 20,000 | 0.819 GiB | 0.783 GiB |
| 40,000 | 1.603 GiB | 1.558 GiB |

At 20k/40k the logical export remains about 306/611 MiB. Generated `model.c`
is byte-identical. Sampled stage RSS at 40k: native capture 1.25 GiB (before
1.37), stream/decode 1.29 GiB (before 1.58), DB import 1.51 GiB (before 1.60),
typed lowering 1.51 GiB and C rendering 1.56 GiB (C rendering is now
1.12 GiB). Between these measurements and `00485642` the wrapper's glibc page
release was silently compiled out (its `__GLIBC__` test preceded every C
library header), raising the 40k peak to 2.15 GiB at DB import; with it
restored the 40k peak is 1.514 GiB. These are
generation-process peaks, not generated-simulator runtime memory. Export size
is approximately linear for this corpus; other shapes can differ.

Packing the staged `SemanticNode` (booleans into one flag word, `u32`
references, boxed rare attributes; 248 to 160 bytes) lowered the sampled
stage peaks on 2026-10-05 (release, medians of three interleaved runs, before
`a0ffc09e`): stream 347 to 297 MiB at 10k and 1,323 to 1,122 MiB at 40k, DB
import 397 to 347 MiB at 10k and 1,550 to 1,348 MiB at 40k. The whole-run peak
did not move (0.388 to 0.386 GiB at 10k, 1.513 GiB at 40k) because typed
lowering (the DB plus typed IR; 395 MiB at 10k, 1,549 MiB at 40k) now sets it
at the same level DB import used to.

### Cause

The [C++ wrapper](../src/wrapper/slang_c_api.cpp) must finish capture while
Slang's compilation is live: it back-patches placeholder nodes, edge roles,
type windows, overridden parameters and lexical bindings. For this corpus each
small register process contributes roughly 60 semantic nodes, 90 edges and 12
constants (about 16 KB of charged export). After capture the compilation is
destroyed and the glibc heap trimmed; the wrapper then streams each table to
the [Rust receiver](../src/ffi/slang/stream.rs) and frees it once delivered
(semantic nodes chunk by chunk), so native and Rust copies overlap only per
table. The receiver interns node names and kind spellings.

[`Db::from_slang`](../src/core/db/readme.md) needs random access to the whole
node set (child flattening, array-select chains, full names), so the receiver
stages the owned snapshot (160 bytes per semantic node) and the snapshot and DB
overlap during import. The driver releases the snapshot after import and
consuming generation releases the DB after typed lowering.

Remaining overlap: Slang compilation and the capture tables during capture;
the staged snapshot and DB during import; the DB, semantic origins and typed IR
during lowering; execution IR, rendered artifacts and the assembled model text
during rendering. Sampled stage RSS includes all live representations and
allocator-retained pages; it is not an exclusive allocation total for that
stage.

The simulator's capture has no export byte budget: `Limits::simulator()`
leaves export, source and value-bit bytes unlimited and record counts at their
structural ceilings (32-bit record IDs), so only available process memory
bounds a large design. Library and LSP callers keep their bounded
`Limits::default()` budgets, where `export byte limit exceeded` can still occur.

### Intended direction

Lowering now sets the whole-run peak, so lower the DB plus typed IR it holds
(compact DB nodes with side tables for rare kind payloads), or import nodes
incrementally to shrink the staged snapshot further (the remaining 160 bytes
are mostly the 32-byte `Option<SourceRange>` and three 16-byte interned
texts). Rendering memory is tracked in the C emission entry below.
Preserve checked C ABI ownership and the single owned DB import; consumers
must not traverse native ASTs independently. Verify exact values, source
identity and diagnostics as well as generated-model behavior.

Simulator compiles have no export budget; use `LLG_MEMORY_LIMIT_MB` for the
optional process-wide memory guard, budgeting for the measured frontend peak
rather than the exported byte count.

### Reproduce

Run:

```sh
/usr/bin/time -v llg --gen-only \
    --top many_processes_registers_config \
    --define LLG_CORPUS_N=40000 --define LLG_CORPUS_EDGES=2 \
    --out-dir <dir> perf/corpus/many_processes.sv
```

Read maximum resident set size from `time`. Use 5k/10k/20k/40k processes, at
least three runs per point, release binaries and medians. The Linux runner
`python3 perf/scripts/frontend_scale.py <release-binary> <evidence-dir>` records
GNU time, stage markers, sampled RSS and generated-C hashes, running points
serially. See [profiling](../perf/README.md#frontend-stage-scaling). Export
bytes count captured data, not the bytes of generated `model.c`.

## High generation memory use during C emission

**Status:** open, narrowed; C rendering no longer sets the generation-process
peak for the measured corpus.

### Symptom

Rendering the generated C model still adds memory over the execution IR,
linearly in design size, but stays below the frontend and lowering stages.
Linux x86-64 release, `many_processes_registers_config`, two clock edges,
medians of three interleaved runs on 2026-10-04 (before `b0a3a50a`, after
`00485642`), sampled stage RSS:

| Processes | Execution | Render before | Render after | Whole-run peak before / after |
| --- | --- | --- | --- | --- |
| 5,000 | 144 MiB | 216 MiB (+66) | 157 MiB (+13) | 0.271 / 0.201 GiB |
| 10,000 | 272 MiB | 414 MiB (+132) | 295 MiB (+23) | 0.529 / 0.388 GiB |
| 20,000 | 530 MiB | 809 MiB (+265) | 576 MiB (+46) | 1.045 / 0.764 GiB |
| 40,000 | 1,055 MiB | 1,607 MiB (+543) | 1,145 MiB (+90) | 2.151 / 1.514 GiB |

Allocator-counted live heap at 10k: the render peak fell from 323 to 148 MiB
over a 93 MiB execution IR. The before peaks were raised by a lost glibc page
release in the wrapper (see the frontend entry); the after peak is DB import
or typed lowering. Render time is unchanged within host noise; whole-run
generation time is equal or lower. Generated `model.c` and
`model.symbols.tsv` are byte-identical at every size and across all simulator
fixtures in both optimizer modes.

### Cause

Every rendered coroutine body is kept until exact sharing has seen all of
them, because a group's membership and frame-type names are only known at the
end; for this corpus those bodies are about half of the remaining increase.
The rest is per-coroutine frame layouts and descriptors, the shell of the
execution IR (signals, functions, process shells; process operations are
released as each body is rendered) and the assembled model text. Sampled stage
RSS includes allocator-retained pages and is not an exclusive allocation
total for the stage.

### Intended direction

Normalize each body for sharing as soon as it is rendered and drop
duplicate members' text, reconstructing it exactly only for groups below the
sharing threshold; this requires abstracting frame-type names
that are resolved after rendering. Stream the final text to `model.c` only if
the generated-model API stops returning it in memory. Generated `model.c` must
stay byte-identical, including both optimizer modes and the exact sharing
output; see the [emitter guide](../src/sim/emit_c/model/readme.md#generation-memory).

### Reproduce

Use the command and runner in the
[frontend memory entry](#high-frontend-memory-use-during-slang-wrapper-capture-and-import)
and compare the `execution`/`optimization` and `render` stage RSS that
`perf/scripts/frontend_scale.py` records.

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

The 2026-10-04 record compaction (preceding entry) also shortens owned DB
import and typed lowering. Interleaved three-run medians at 40k: import
7.07 → 2.72 s, lowering 9.81 → 5.85 s, C rendering 11.47 → 12.47 s, whole
generation 58.3 → 53.3 s wall (52.5 → 49.1 s user); Slang analysis still
dominates and its time varies with host load.

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

## Long bodies that cannot be outlined compile superlinearly

**Status:** narrowed. Statement runs of long process, function, task and fork
branch bodies are outlined into helper C functions
(`src/sim/emit_c/owned/outline.rs`, tunables `LLG_OUTLINE_RUN_WEIGHT` and
`LLG_OUTLINE_MIN_RUN_WEIGHT` in `src/sim/ir.rs`); the bodies listed under
Cause still become one C function each.

### Symptom

GCC and Clang optimize a function in time superlinear in its size. Before
outlining, the four `rtl_006` constant-operand bodies in one `initial` block
(`--no-opt`, 33,000 lines of C) built in 27.7 s with GCC 14 `-O3` against
18.0 s for the four widths built separately; with outlining they build in
6.8 s against 8.0 s (Clang 19: 37.3 s → 10.5 s). The run weight counts the C that
formatting arguments and block scopes render, so helpers stay under about
1,800 lines in both optimizer modes; counted by IR nodes alone, the optimized
rows became a 4,610-line helper that MSVC `/O2` did not build within 180 s.
Bodies outside the cases below now build in time about linear in their length (see
[performance trade-offs](sim_performance_tradeoffs.md#long-bodies-as-helper-functions)).

### Cause

A run is outlined only when no state of the body crosses into it, so these
stay in one C function:

- statements that read or write a local or formal of the enclosing body, and
  statements that suspend, fork, disable, or call a subprogram that may
  suspend or disable;
- one statement whose nested statement lists are each shorter than a run,
  such as a `case` with thousands of short arms;
- bodies that instance sharing may merge (at least `LLG_SHARE_MIN_INSTANCES`
  processes from one source location), recursive functions, continuous
  assignment evaluators, callbacks and storage initializers.

### Intended direction

Pass the live body locals to helpers by address, and group consecutive light
arms or iterations of one statement into helpers, when a design shows the
cost; both change the generated C of bodies that build in acceptable time
now.

### Reproduce

Write an `initial` block of 3,000 32-bit assignments to one automatic
variable declared in the block, run `llg --no-opt` with `LLG_BUILD_TIMINGS`
set, and compare `build_ms` with the same statements assigning a module
variable: 9.2 s (one function) against 3.4 s (44 helpers; 8.6 s before
outlining), GCC 14 `-O3`, one CPU.

## Gate-level netlist generation memory

**Status:** open for generation memory; the per-instance C data and the
quadratic generation stages are fixed.

### Symptom

The C model now grows by about 530-750 bytes per cell (it was 6.5-7.5 KB), and
gcc builds a 64,000-cell model in 10-13 s instead of 10-22 minutes. Generation
peak RSS fell only by about a fifth, because it is dominated by the frontend,
the owned database and the lowered IR, not by the C text. Release
`--gen-only` on generated netlists (a buffer chain, and hierarchical blocks of
a cell library with specify paths and timing checks), 2026-10-23:

| Cells | `model.c` MB before -> after | Peak RSS MB before -> after |
| --- | ---: | ---: |
| chain 64,000 | 418.1 -> 34.0 | 2,379 -> 1,907 |
| hier 64,000 | 474.8 -> 47.1 | 2,841 -> 2,294 |
| chain 256,000 | 1,686.2 -> 139.5 | 9,086 -> 7,225 |
| hier 256,000 | 1,916.8 -> 193.3 | 11,075 -> 8,871 |

At 64,000 chain cells RSS reaches 1.1 GB in the native frontend, 0.97 GB at
the end of the database import, 1.63 GB at the end of lowering (database,
semantic model and IR live together), and 1.9 GB while rendering, where every
process body is rendered before sharing replaces most of them.

### Cause

Lowering builds one IR process, signal, net group and path set per instance
(`IrStmt` and `IrSignal` are 224 bytes each), and the native frontend and the
owned database hold the elaborated instance tree. Per-instance C is now table
data: net shapes and pools, per-module path and timing-check templates, one
descriptor and spawn row range per shared body class and one record row per
instance ([instance sharing](../src/sim/emit_c/model/sharing.rs),
[instance tables](../src/sim/emit_c/owned/model/initialization/instance_tables.rs)).

### Intended direction

Lower each cell definition once and instantiate it from per-instance
connection data, so the IR and the rendering buffers grow with definitions
rather than instances; render and normalize process bodies as a stream so
duplicates are dropped before the next one is rendered.

### Reproduce

Generate a buffer chain (`module tb; reg a; wire y0; BUF u0(.a(a), .y(y0)); ...`
with a one-path `BUF` cell) at 64,000 cells and run `LLG_PROFILE_STAGES=1
llg --gen-only` while sampling `/proc/<pid>/status`. `tests/sim_scale.rs`
caps the time growth and the C bytes per cell of a 300 -> 1,200-cell netlist.

## Remaining non-flattened fixed-value contexts

**Status:** open; RTL-002 and RTL-002b implement descriptor transport for
integral fixed arrays, and RTL-101/RTL-101b column layout for large records.

### Symptom

Integral variable arrays copy, compare, select rows, merge conditionals, stream
(including multiple segments and unaligned slices), initialize and pass through
input/output/inout/ref formals and returns of static, automatic and recursive
functions without becoming one packed value. Array-valued pattern items and
pattern-lvalue row scatter use the same views, and so do arrays of unpacked
records whose elements fit the packed limit (RTL-099 qualifies 1,048,576
records). Unpacked records with a member array above the 4,096-cell dense
threshold, and records or finite tagged unions wider than the packed limit,
keep each member array and scalar leaf in its own descriptor column (RTL-101):
they copy, compare, merge conditionals, match patterns, pass through module
ports and subroutine formals/results/locals, and a 65,537-cell member generates
about 10 KB of C instead of 79 MB. RTL-101b adds whole-value pattern bindings,
record call results compared or selected inside an expression, static and
automatic declaration initializers, uniform member initializers, `real`,
`string` and `chandle` members (a SIM-003 native value per subroutine record)
and tag checks on whole member copies. Column records still reject: selecting
a whole member array, a sub-record, or a
`chandle` member of a call result inside an expression, and comparing a
record pattern, conditional or tagged expression operand (only storage and
calls compare); and a whole-value binding in a continuous assignment. A nonblocking write of a whole
tagged-union member array checks the tag when it is issued; element writes
recheck at commit. The oversized source of a whole-value `matches` binding that
is a fixed array rather than a record still rejects (RTL-016).
Dense rows as descriptor pattern items and scatter targets, and dense arrays,
packed values, runtime `with` ranges and nested streams as parts of an
oversized stream, use descriptor transport (RTL-103); resizable container
operands, plain or `with`-selected, stream as runtime-sized cells (SIM-020).

Fixed-array `reverse`/`sort`/`rsort`, selected-row reductions and `inside`
over stored cells (descriptor arrays, selected rows and dense arrays above 16
elements) run as one cell-wise loop (RTL-014). A sort `with` key reads each
element as one item value, so a key over rows wider than the packed limit
rejects with a diagnostic; reductions whose item is such a row and value
receivers without stored cells (oversized call results, casts and slices) keep
the payload limit. Small automatic arrays and non-ref formals are packed values
and keep the straight-line compare-exchange schedule, which is quadratic and
bounded by its comparison limit. A sort keeps one key per element and
materializes every descriptor cell it moves.

Casts between descriptor-backed arrays reshape and apply two-state conversion as
descriptor values. Arrays above the dense-cell threshold but within the packed
limit still flatten outside equality and descriptor-destination assignment: a
reshaping cast into a dense destination, mid-size conditionals passed to packed
formals, and mid-size function returns expand every cell into the generated C
(megabytes at 5,000 cells). Correctness is unaffected.

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

### Cause

Descriptor transport and column layout cover the contexts listed above; the
call ABI passes values within the packed limit as packed values, so mid-size
arrays expand every cell, and each call or view operand takes its own
snapshot. The rejected column-record forms have no descriptor view yet.

### Intended direction

Widen the descriptor call ABI below the packed limit, and give the rejected
column-record selections and comparisons descriptor views.

### Reproduce

`tests/fixtures/sim/feature_completion/rtl_101b/neg_call_member_array.sv` and
`neg_continuous_whole_binding.sv`;
`tests/fixtures/sim/feature_completion/rtl_014/neg_row_key_capacity.sv`; for
the flattening cost, a 5,000-cell `int` array passed through a packed formal
with `--gen-only`, then measure `model.c`.


## Native record values outside by-value subroutine storage

**Status:** open; SIM-003 implements descriptor-backed native records for
subroutine formals, results and locals; SIM-006 stores them as elements of
queues, dynamic and associative arrays; SIM-007 adds fixed
arrays of strings, handles and native records (multidimensional ones as one
row-major view), run-time indices into native
member arrays, packed-member selects, record-element equality and tagged
unions with native members in module/static variables, container members
of module records and of subroutine record values, native records, strings
and handles declared in procedural blocks, member defaults and any record
source as a module-record initializer, and member-wise equality of nested
records and member arrays; SIM-008 binds native record `ref` formals to whole
subroutine records and to module, static and procedural-block records;
retained element references bind them to fixed-array, queue, dynamic and
associative array elements, and `ref` formals of any kind to members of those
elements and of subroutine records, constant or run-time indexed;
FIN-001 adds nonblocking writes to members of fixed-array record elements, of
records with container members and of static subroutine records.

### Symptom

Unpacked records with string, real, chandle or class-handle leaves copy,
compare and cross input/output/inout formals and results as runtime values,
are elements of resizable containers and of one-dimensional fixed arrays.
Queue, dynamic and associative members of records
(`struct { string s; int q[$]; }`) are their own containers in module and
static records and travel as companion containers of subroutine record values
(formals, results, automatic and static locals): methods, selects, `foreach`,
whole-record copies, ports, conditionals, equality (except associative
members) and assignment patterns work. As elements of queues, dynamic,
associative and fixed arrays such records keep queue and dynamic-array
members inside each element value: whole-element copies, calls and
equality work, and assignment, call and system-task statements that name an
element's member (`list[i].q.push_back(x)`, `x = list[i].q[j]`) copy that
member out of the element and back, which costs time proportional to the
member's size per statement. These legal forms still reject with
explicit diagnostics: equality of rows of multidimensional fixed arrays of
strings, handles or native records, and such rows as `ref` actuals;
associative members of container or fixed-array record
elements; an element's container member changed by an expression outside an
assignment, call or system-task statement (a condition or loop header that
pops it), or named as the target of a timing-controlled assignment or a
`ref` actual (reads in conditions, loop headers,
`foreach`, initializers and timed assignment values copy the member at each
evaluation); instance class properties of a record type with container
members (instance properties of records with string, real or handle members
are one value per object, records of integral leaves are fixed-width
properties, SIM-011, and static record properties are one shared record); equality of records with an associative member (and conditionals
with an ambiguous predicate on such records);
a run-time selection among more than 4,096 leaves of a module record
(`NATIVE_MEMBER_SELECT_LIMIT`), and among more than 256 whole records of any
record (`NATIVE_RECORD_SELECT_LIMIT`); a task waiting on a `ref` formal bound
to a member of a subroutine record whose type has container members (the
formal takes the record's companions, whose changes publish nothing it
follows); a queue or dynamic-array member below an element of a member array
of 16 or more cells, which a subroutine record holds in its own slot,
anywhere but through a `ref` formal bound to the element
(`t.e[1].q.push_back(x)`, `x = t.e[1].q[0]`, `v = t.e[1]`, `%p` of
`t.e[1]`; located at the selection, since the member is a nested slot of the
record's value that only the reference mirror reaches, about two days); as a
`ref` actual, a run-time-indexed record element
of a module, static or block record of fewer than 16 cells bound by a function
called in a condition, a loop header or another statement that is not an
assignment without timing control, a `return` or a system-task statement, or
after an operand of the same statement that may write the index, or
among more than 256 elements or combinations
(a call statement, or such a statement around a function call, lowers once per
element, `tr(o.e[i])`, `x = f(o.e[i])`; a task that waits on
the formal stops the run if the index selects no element); an event control on
a member of a native record `ref` formal when the actual is a subroutine record
(a generic lowering
diagnostic); a queue, dynamic or associative member of a module record element selected at
run time (`m.e[k].q`, "unsupported object method receiver" at the select; a constant `m.e[1].q` works,
as does a run-time element of a fixed array of records, `a[k].q`, because each
element of a module record owns its own container and the select would need a
run-time choice among them, about three days); sub-record, member-array or handle selects of a native call
result (`f(...).inner`; scalar members such as `f(...).s` work); native outputs
bound inside an expression (call them as a statement instead); record ports
whose type has a built-in semaphore, mailbox or process member (those handles
publish no change marker) or a chandle member (chandle ports themselves are
illegal, SV 6.14, and value links carry no chandle leaf); an automatic record, string, chandle or class
handle declared in a procedural block that a `join_any` or `join_none` fork
can keep live while the block is entered again (declared in such a fork that
runs again, such as the `for (...) fork automatic string s = ...; join_none`
idiom, or in a block that runs again and starts such a fork reading it); a
member default whose value is itself a record with native members (its
frontend constant is not captured). Tagged unions with real, string, record or
class-handle members execute as module, static and subroutine values
(formals, results, locals, variable ports, conditional operators) and as
elements of one-dimensional fixed, queue, dynamic and associative arrays; a
checked member access of an element whose index has side effects, unions
nested in a record or a multidimensional array, nonblocking writes to one
member (SV 11.9 checks them against the tag at commit, and queued native
writes carry no such check) and nonblocking writes to subroutine storage
reject; whole module and static variables take nonblocking writes. String and real pattern variables bind
in process and subroutine bodies; handle bindings and structure patterns over
native records reject. A missing associative record element
compared with `==` reports the SV 7.8.6 warning once per member.

### Cause

A native value is one rooted `llg_value_t` tree; leaves are addressed by
constant item paths resolved at lowering time. Module-level native records keep
their per-member lowering, so whole-value transfers between the two
representations go leaf by leaf, and a run-time member index is a bounded
decision tree over the declared leaves (a module record's leaves are separate
objects with their own change markers, so no single table addresses them). Fixed arrays of native elements
are fixed-size views of the container runtime; a row of a multidimensional
one is a run of cells, not a view of its own. A record member that is itself a
resizable container is a companion container of a native value, but a nested
dynamic array inside a container element's value: no container operation
addresses that nested slot in place, so a statement stages a copy. A record, string or handle declared in a procedural block reuses
module storage, one copy per declaration and instance: an automatic one is
reset at each block entry, so a second live activation has no storage of its
own. The Db captures member defaults as constants, and an unpacked record
constant has no captured value. A native `ref` formal is the caller's
`llg_value_t` and companion containers when the actual is a whole subroutine
record; a module-like record has no such value, so a call binding one runs a
specialization of the subroutine that names the record's leaves directly,
which needs the record statically: a run-time record element lowers the call
once per element under a decision tree, and only a statement can branch that
way, so a function call in an expression lowers its whole statement per element
with the index taken when the statement starts, before its other operands,
and only for a statement that evaluates its expressions once (a scalar leaf is
chosen among stable addresses at the call). An element
reference shares the element's item array; a queue or dynamic-array member is
a nested slot of another representation, so the call mirrors it in a companion
container, at the member's size per change in either direction. A member of a
subroutine record passes the record's own companions, or mirrors the nested
slots of a member array the record holds in its own slot (16 or more cells).

### Intended direction

An addressed-container operand for container members of elements (in place
of statement staging); a direct run-time item index
for member selects of subroutine records, whose elements share one value; a
commit-time tag guard on queued
native writes for tagged-union member NBAs; and per-activation native
roots for forked automatic block records, strings and handles.

### Reproduce

`tests/fixtures/sim/feature_completion/sim_003/neg_native_*.sv`,
`sim_008/neg_record_ref_element_select_value.sv`,
`sim_008/neg_record_ref_subroutine_slot_container.sv`,
`sim_007/bad_block_record_*.sv`,
`sim_007/bad_block_native_*.sv`, `sim_007/bad_record_member_default.sv`,
`sim_007/bad_record_element_*.sv`, `sim_007/bad_record_container_element.sv`,
`sim_007/bad_module_record_element_queue.sv`,
`sim_011/neg_record_container_property.sv`, `sim_007/bad_multidim_row_*.sv` and `sim_007/bad_tagged_*.sv`.

## Resizable containers at subroutine, object and nesting boundaries

**Status:** open for the forms below; every other container form at a
subroutine, object or nesting boundary works. See
[sim_features](sim_features.md).

### Symptom

These legal forms reject with explicit diagnostics:

- a class property read outside a procedural statement (`assign n = h.q.size()`);
- a class property selected through a computed handle in a `wait`, whose
  condition is evaluated again after the statement that read the handle ended;
- `foreach` over a keyed or nested array whose handle comes from a call;
- `foreach` with a dynamic level below a string key and a queue or dynamic
  array (`foreach (s[k, i, j])` over `int s[string][$][$]`);
- a `ref` actual that is a queue or dynamic array nested in a container
  element, for a task or function formal, `const` or not, reported at the
  actual, and a `ref` actual that is a queue or dynamic-array member of a
  container record element;
- `%p` of a nested queue or dynamic array in a report formatted after its
  statement (`$strobe`, `$monitor`);
- a `default:` key in a pattern for a nested container element;
- a `ref` formal named in a plain `fork`/`join`.

### Cause

Container operations name their storage by a container index. A property
selected through a plain handle read is an alias that re-reads the handle at
each use (SIM-011). A nested queue, dynamic or associative array is bound in
place to a lexical alias that pins it for one statement, so a report
formatted later has no alias. A `ref` formal takes a nested associative
array; for a nested queue or dynamic array the binding of a reference that
outlives the statement (a task that suspends, a change of the element through
another name) is not decided, and the actual rejects.

### Intended direction

A `ref` to a nested queue or dynamic array through the in-place alias, with
the pin held by the formal (SIM-008); fork capture pins (SIM-010).

### Reproduce

`tests/fixtures/sim/feature_completion/sim_006/neg_*.sv`.

## Remaining forms of unpacked-array container elements and multidimensional class properties (SIM-011, SIM-027)

**Status:** open; FIN-001 C1 serves cells of unpacked-array elements of
queues, dynamic and associative arrays (`da[k][j]`, `aa[k][j] = v`, also below
record members at run-time indices), whole elements of integral or real cells
to and from array variables and patterns (`row = da[k]`, `da[k] = row`,
`aa[k] = '{...}`), rows of multidimensional class properties of packed cells
(`o.g[i]`), and writes to a member of a packed element of a class array
property (`o.a[i].m = v`).

### Symptom

These legal forms reject with a located diagnostic: a whole element of string,
handle or record cells copied to or from an array variable (`sr = sd[k]`); a
row or slice of a larger array copied to or from a whole element
(`m2[1] = da[k]`); a row of a multidimensional class property passed to a
fixed-array formal or compared with `==`, and a bit select of one of its
elements (`o.g[i][j][3]`); a member of a packed element of a class array
property as an output or inout actual; and a multidimensional class property
of real, string, handle or record cells (`static real g[2][2]`), which rejects
at its declaration. A whole member array read from an
element (`x = oq[0].arr`, `x = oq[0].e[1].arr`) fails without a location
("hierarchical reference ... is not supported").

### Cause

Rows of string, handle or record cells live in a fixed view of the container
runtime (a descriptor array), and the element's value holds its cells as items
of one native value; no transfer between the two exists. Rows of a larger
model array have no whole-array view. A class property row has no flattened
value for a formal, and the packed-element member write is a statement form.

### Intended direction

A transfer between a descriptor-array range and a fixed-array item of a
native value, and the row operand for fixed-array formals and equality.

### Reproduce

`neg_container_native_row.sv` and `neg_container_row_slice.sv` in
`tests/fixtures/sim/feature_completion/sim_027/`; `neg_multidim_property_row_value.sv`,
`neg_multidim_property_row_equality.sv`, `neg_multidim_property_cell_select.sv`,
`neg_multidim_property_real_cells.sv` and `neg_class_packed_member_actual.sv` in
`tests/fixtures/sim/feature_completion/sim_011/`.

## Bit-stream limits (SIM-020)

**Status:** open; SIM-020 streams strings, resizable containers and records
with such members through runtime-sized bit streams.

### Symptom

These legal forms reject with explicit diagnostics: streaming a class
object's members (SV 11.4.14.1); a bit-stream cast into a struct with string
or resizable members; a nonblocking unpack into a whole dynamic array, queue
or string; a dynamic array or queue whose element is an unpacked array as an
unpack target (streaming it as a source works); and an unpack whose targets
together exceed the packed width unless the target is one whole fixed array.

### Cause

Class objects have no owned stream descriptor. A cast into a record with
dynamic members needs a greedy unpack into its member leaves, which the record
assignment paths do not lower. Nonblocking container writes have no queued
whole-container commit. Unpacked-element containers have no element writer
for streamed cells, and multi-target unpacks consume one packed value.

### Intended direction

Generalize the bit-stream unpack (`llg_bitstream_t` plus typed target
segments) to record leaves, unpacked-element containers and several oversized
targets, and queue a whole-container commit for nonblocking unpacks.

### Reproduce

`tests/fixtures/sim/feature_completion/sim_020/unsupported_*.sv`.

## File input destinations not supported (SIM-026)

**Status:** open. SIM-026 accepts every other legal destination of
`$fscanf`, `$sscanf`, `$fgets` and `$fread`.

### Symptom

- A field of a packed member of an unpacked structure that also has a string
  or real member (`us.p.lo`, where `p` is packed and `us` has a `string`
  member) is rejected with `cannot resolve hierarchical assignment LHS`. Plain
  assignments to this form are rejected in the same way. The whole member
  (`us.p`) and structures without string or real members work.
- A character of a string (`s[0]`) as a destination is rejected explicitly.
- Reading an element of an integer-keyed associative array of strings
  (`string m[int]`) is rejected with `string container read requires a
  dynamic array or queue`. A scan into `m[3]` stores correctly; only reading
  it back fails, and string-keyed arrays work.
- `$fread` into an unpacked structure is rejected by the frontend
  (`invalid argument type`). The LRM names only integral variables and
  memories.

### Cause

Structures with string or real members are stored as native records, and
their lvalue path resolves one member level only. A string character has no
retained element cell to stage a conversion through. String reads from
associative arrays are lowered only for string keys.

### Intended direction

Resolve nested packed fields of native-record members as selects of the
member. Stage a string-character destination through a byte local, as
container string elements are staged. Add integer-key string reads to the
associative-array read path.

Binary input and output assume a little-endian host. `%u` and `%z` (in
`$fscanf` and `$fwrite`) move 32-bit words as little-endian bytes. That is the
"native endian format" of SV 21.3.4.3 on every supported target (x86_64 and
arm64 on Linux, Windows and macOS). The `%u` writer and the `%u`/`%z` reader
(`llg_scan_binary` in `src/sim/rt/scheduler/scanning.c`) shift bytes into
little-endian order explicitly. The `%z` writer (`llg_format_raw4` in
`src/sim/rt/scheduler/formatting.c`) copies host words with `memcpy`. A
big-endian port must make all three follow the host order.

### Reproduce

`tests/fixtures/sim/feature_completion/sim_026/neg_string_character.sv`. The
other forms reproduce with one call each, for example:

```systemverilog
typedef struct packed { logic [3:0] hi, lo; } p_t;
typedef struct { p_t p; string s; } u_t;
u_t us; string m[int]; integer c;
initial begin
  c = $sscanf("5", "%h", us.p.lo);                 // rejected
  c = $sscanf("w", "%s", m[3]);                     // stores
  $display("%s", m[3]);                             // rejected
end
```

## Waveform dump coverage limits (SIM-047)

**Status:** open; SIM-047 qualified the dumped catalog and rejects the
unsupported targets explicitly (decision S47-D9).

### Symptom

- By decision (2026-11-10), container cells and the members of records held in
  a container are not dumped in any format: a scope dump leaves out queues,
  dynamic and associative arrays, strings, class handles, and record members
  stored in one (`rows__iN__n` of a 16-row member array), without a message;
  naming one in `$dumpvars` is an error at the argument. VCD and FST files
  list the same variables.
- An unpacked structure variable declared in a block appears as one packed
  vector, not as a scope of its members.
- An unnamed block directly inside a task or function adds no
  `unnamed$$_N` scope: its variables appear in the subroutine's scope.
- An FST file flushed by `$dumpflush` cannot be opened by GTKWave's reader
  until the run closes it.

### Cause

Registration happens once, before the dump starts, into one catalog both
writers read; a container's cells come and go at run time and have none.
Block structures live in one flattened signal, and a registration
names a whole signal. The owned DB parents variables of such an unnamed block
to the subroutine. libfst writes the hierarchy and block index at close.

### Intended direction

A registration with a bit range serves the flattened structures. The FST
behaviour follows the format.

### Reproduce

`flush_read.sv` with `FILE="trace.fst"` (`sim_047::a_flushed_dump_is_readable_while_the_simulation_runs`);
`containers.sv` and `neg_container_dumpvars.sv`, each with a `.vcd` and a
`.fst` file (`sim_047::vcd_and_fst_dump_the_same_variables_and_no_container_cell`,
`sim_047::naming_a_container_is_the_same_located_error_in_both_formats`).

## Strength-only events a reader does not see

**Status:** open; these forms wake on value changes only (T1-D8).

### Symptom

- `@(n[i])` on an element of a net array with a dynamic index does not run on
  a strength-only change of the selected cell, and `%v` of `n[i]` shows the
  strong strength of its value. Dynamic bit and indexed part-selects of a
  vector net do (T1-D12).
- An event control in a task or function body on such a net, even `@(w)`,
  does not run on a strength-only change; the same control in a process does.
- A net on the sink side of a port between nets of different widths shows and
  wakes on the strong strength of the port's link, not on the source's, when
  the port cannot merge: a net delay on either side, a port strength, specify
  paths or SDF, `--vpi-port-delays`. Other such ports carry the strength
  (T1-D13).

### Cause

A dynamic select reads the bytes of its net's strength view that the lowered
packed select names; an element of a net array lowers to an array read, whose
cells have one view each. Subroutine bodies are lowered before the processes
that collect net drivers, so when their event controls are lowered no net is
known to vary in strength. A port that cannot merge stays a continuous
assignment, whose driver has one strength.

### Intended direction

Select among the cells' views by the array index; mark the varying nets before
subroutine bodies are lowered. A port that cannot merge needs strength carried
through its link, which only matters for a design reading strength across one.

### Reproduce

Two drivers of different strength on `n[1]` of `wire n[1:0]`, `i = 1`, and
`always @(n[i]) $display("%v", n[i])`; a task that loops on `@(w)` for a
one-bit net `w` with such drivers; that net on a two-bit input port whose
formal declares a net delay, with `always @(i)`.

## Strength views past the packed width limit

**Status:** open by decision (T1-D8); the form stops code generation with an
error that names the net and the limit.

### Symptom

A reader of a net's strength-only changes (an event on the net or a select of
it, `@*`, `always_comb`, `always_latch`, `wait`, a `$monitor` argument) or a `%v`
of it rejects when the net has more than 131071 bits. The net compiles while
nothing reads its strength this way.

### Cause

A strength view takes eight bits per net bit and is one packed signal, so the
packed width limit of 1048575 bits applies to the whole net, even when the
reader selects a few bits of it. Waking on values alone would lose runs without
notice.

### Intended direction

Allocate the view as bytes outside packed storage, which only matters for a
design reading the strength of such a net.

### Reproduce

`tri1 [131071:0] w; always @(w) $display("woke");`
(`T1-D8_wide_strength_view_rejected`).

## Sibling ports with different net delays

**Status:** open by decision (T1-D4); the form stops code generation with a
located error. It compiled before T1-D4, silently without a delay.

### Symptom

Two or more ports merge into one net, their formals declare different net
delays, and no dominating declaration has a delay:

```systemverilog
module leaf(n); inout n; wire #D n; endmodule
wire e;
leaf #(3) a(.n(e));
leaf #(2) b(.n(e));
```

llg stops with `net propagation delay for ... has conflicting member delays`.

### Cause

23.3.3.7 gives the simulated net the delay of the dominating net, and T1-D4
falls back to the outermost declared delay. Siblings are equally far out, so
the fallback names no single delay; one simulated net can hold only one.

### Intended direction

Keep the rejection until a rule is defensible. A simulator tried gives none:
with the formals read hierarchically the net takes the largest delay on every
transition, without such a read later transitions take no delay at all, a
first value set at time 0 took 2, 6 and 8 for sets of delays where another
design took 3, 4 and 5, and rise and fall delays do not combine per transition.
Taking the largest delay or none would both invent a behaviour. The workaround
is to give the dominating (outer) net a delay, or the same delay to every
formal.

### Reproduce

`T1-D4` covers the accepted forms; the snippet above rejects.

## randsequence limits (ADV-016)

**Status:** open (ADV-016); the forms below stop code generation with a
located error, and everything else in SV 18.17 runs.

### Symptom

- A production argument that is not `input` (`output`, `inout`, `ref`) is
  rejected: "formal `o` is not an input; llg supports only input production
  arguments".
- `rand join` requires distinct productions that are not recursive and do not
  generate another production of the same join.
- A recursive production (one that can generate itself) saves its arguments,
  its rules' implicit variables and its result for every active generation;
  only packed, real and string values can be saved, so a class handle,
  chandle, unpacked array (including the implicit array of a repeated
  value-returning production) or unpacked structure there is rejected.
- Recursion deeper than 10000 active generations of recursive productions is a
  fatal run-time error (exit status 1) located at the production item.

### Cause

Productions are expanded into the enclosing process's continuation, so
arguments are copied in once and nothing is written back. A joined sequence is
interleaved element by element from the enclosing process, which needs each
joined production expanded in place. Recursive generations push their values
onto a packed queue and a string queue held in the activation frame; other
storage classes have no saved form yet.

### Intended direction

Copy-out of output arguments at production exit, a segmented join driver for
recursive joined productions, and frame entries for handles and unpacked
values through the native container element writers.

### Reproduce

```sh
scripts/run-tests.sh -E 'binary(sim_feature_completion) & test(adv_016::)'
```

## Module path limits (ADV-007)

**Status:** open (ADV-007; later work in ADV-010 to ADV-012 and ADV-019); the
forms below stop code generation with a located error, and simple module
paths otherwise run.

### Symptom

- A variable destination (`output reg q` with `(a => q) = 1;`) is rejected:
  "a variable destination is not simulated yet; declare the output as a net".
- An inout destination that is also driven outside the module is rejected:
  "the destination is also driven outside the module, so the path would delay
  that driver too".
- A bit or part select of a multidimensional packed terminal is rejected.
- State-dependent and edge-sensitive paths and pulse control run since
  ADV-008, the basic timing checks since ADV-009 and the combined and
  negative ones since ADV-010. The delays of delayed timing check signals
  are fixed when the model is generated: annotating new `$setuphold` or
  `$recrem` limits changes the windows but not the delays, and a check whose
  negative limits sum to 0 or 1 is not in the model at all.
- A path whose terminals only partly overlap an `ifnone` path's (for example
  `ifnone (a *> y)` and `if (c) (a => y[0])`) does not suppress it: an
  `ifnone` path yields only to active paths with exactly its terminals.
- VPI exposes paths as `vpiModPath` objects and their delays through
  `vpi_get_delays`/`vpi_put_delays` (VPI-T1); see the VPI limits for what
  stays out.

### Cause

A path delays the destination net group after its drivers resolve, so it
can model a destination whose drivers are all inside the module, but not a
variable (no net group) or an inout net with an outside driver, whose
contribution must stay undelayed.

### Intended direction

Delay only the in-module driver contributions of a shared inout net, give
variable destinations their own propagation record, and add the path forms
of ADV-008 and the VPI objects of ADV-019 on the same per-path tables.

### Reproduce

```sh
scripts/run-tests.sh -E 'binary(sim_feature_completion) & test(adv_007::)'
```

## SDF reading and annotation limits (ADV-011, ADV-012)

**Status:** open. llg reads OVI SDF 2.1 and 3.0 files when it generates the
model and applies them from `$sdf_annotate` and `--sdf` (ADV-012); the
remaining limits are below.

### Symptom

- Only the OVI SDF 2.1 and 3.0 grammars are implemented. Files declaring
  1.0 or 2.0 are read with the 2.1 grammar and 4.0 (IEEE 1497) files with
  the 3.0 grammar, with a warning; IEEE 1497 constructs such as LABEL
  (specparam annotation, IEEE 1800-2009 32.4.3) and BIDIRECTSKEW
  (`$fullskew`, Table 32-2) are syntax errors, so a file using them
  annotates nothing.
- Numbers keep 18 significant digits and decimal exponents up to 300
  exactly; longer numbers are rejected rather than rounded.
- INTERCONNECT does not check that its source and load are on one net.
  NETDELAY naming a net annotates the input and inout ports of the scope's
  instances whose connection is the net or a constant select of it; a port
  connected through a concatenation or an expression is not reached.
- A load port bit keeps one interconnect delay per transition: the delays
  annotated from several sources of a multisource net combine by
  INTERCONNECT_MIPD (decision A12-D10) instead of delaying each source
  separately as 32.5 describes.
- Interconnect delays apply only to an input or inout port with a net of its
  own that no module path drives; other load ports are reported and not
  annotated.
- RETAIN is read and ignored with a warning (Table 32-1 allows it).
- A `$setuphold` or `$recrem` whose negative limits sum to 0 or 1 has no
  rows in the model unless an SDF entry annotates it (ADV-010), so a plan
  can name it but only an annotated instance applies a new limit to it.
- `$sdf_annotate` arguments must be constant (literals or parameters), and
  the files are read when the model is generated: changing a file after
  generation has no effect until the model is generated again.
- A gate whose delay is computed at run time cannot be annotated (generation
  stops with an error naming it).

### Cause

The IEEE 1497 grammar is not available here; per-source interconnect delays
on multisource nets and specparam re-evaluation need new runtime structures.

### Intended direction

Read IEEE 1497 files with LABEL and BIDIRECTSKEW, and keep one delay per
source and load pair on multisource nets.

### Reproduce

```sh
scripts/run-tests.sh -E 'binary(sim_feature_completion) & (test(adv_011::) | test(adv_012::))'
```

## Process handles in plain handle storage stay allocated until teardown

**Status:** open (SIM-015). The SIM-018 collector reclaims class objects but
not the process handle records their properties pin.

### Symptom

A process whose handle was ever stored into a class property, a record
member, an output, inout or `ref` formal, a function result, a static
subroutine formal or a task variable shared with fork branches keeps its
handle record (a few dozen bytes) until the model is torn down, even after
every such copy is gone. Handles kept only in process variables, by-value
automatic formals and container elements are freed as soon as the last copy
is released.

### Cause

Those storage forms hold plain pointers without reference counts. A handle
entering them is pinned (`llg_process_pin`), so a still-reachable copy can
never observe a recycled identity (SV 9.7).

### Intended direction

Give class properties and native record leaves counted handle slots, or make
process handle records collected objects traced like class objects (SIM-018
already enumerates every opaque slot that can hold one).

### Reproduce

`class C; process p; endclass` with `c.p = process::self();` in a loop that
forks many short-lived children: the pinned handle count grows with the
number of children until the model ends.

## Overlapping unbounded sequence obligations cost one thread each per clock

**Status:** open (SIM-037 follow-up).

### Symptom

An assertion such as `a |-> ##[1:$] b` with `a` high on every clock and `b`
rarely high keeps one pending consequent attempt per clock since the last
`b`. Each clock steps every pending attempt, so run time grows with the square
of the gap length: 20,000 clocks with no `b` take about 23 s of model run time
(quick-profile build; the same as before SIM-037). Memory
stays bounded by `LLG_SEQUENCE_THREAD_LIMIT` (default 1,000,000 live threads),
whose exhaustion stops the run with an error rather than dropping attempts.

### Cause

Every attempt is stepped separately so that each one reports its own pass or
fail (§16.12). Attempts with the same automaton state are not merged, and
merging equal tokens (adding their match multiplicity) inside one attempt
scans that attempt's token list. Tokens with
local variables still allocate their local arrays per copy.

### Intended direction

Group pending attempts of one assertion whose token sets differ only in their
start tick and whose transitions are unbounded or already past their lower
bound, stepping the group once and fanning results out per attempt; index the
per-attempt token merging by (state, edge).

### Reproduce

`tests/fixtures/sim/feature_completion/sim_037/budget.sv` with the loop bound
raised to 20,000 and `LLG_SEQUENCE_THREAD_LIMIT` unset.

## Collected objects keep some runtime objects and defer collection

**Status:** open (SIM-018 limits).

### Symptom

Class objects are reclaimed when unreachable, but mailboxes, semaphores,
named events (including the synchronization object of every `event` class
property, created with its object) and pinned process handle records stay
allocated until the model closes, even when no handle names them. A process that allocates many
objects without suspending (one long zero-time loop) is not interrupted:
collection waits for its turn to end, so its live peak is everything it
allocated in that turn.

### Cause

Mailboxes, semaphores and events are runtime objects with their own
lifetimes (blocked waiters, message queues); the collector treats them as
roots for what they contain rather than as collectable objects. Collection
runs only at scheduler safe points, where no generated code is on the C stack
and every live handle is in storage the collector can enumerate exactly
(frame maps describe suspended frames only).

### Intended direction

Allocate mailbox, semaphore and process handle records on the collected heap
with tracers for their waiter queues; add allocation-site safe points for
long-running turns, which needs frame maps for the running frame.

### Reproduce

`repeat (100000) begin mailbox m = new; end` grows memory until the model
ends; `repeat (100000) begin c = new; end` with no delay reaches its full
peak before the first collection (`LLG_GC_STATS=1` reports `peak`).

## Mailbox message forms without a nested value

**Status:** open (SIM-017); the two message types below are kept as explicit
rejections by design.

### Symptom

These mailbox forms stop compilation with a located SIM-017 diagnostic:

- a message whose type is, or has a member that is, an associative array;
- a whole unpacked fixed-array variable (or a row or slice of one) as a `put`
  source, or a whole one as a `get` destination (assignment patterns and
  record members of array type work);
- `try_put`/`try_get`/`try_peek` with a record, array or selected operand in
  an expression that is not a statement, an assignment, a system-task
  argument, an `if` condition or a loop condition or step (for example a
  `case` expression, a `repeat` count or a variable initializer).

### Cause

A message is a singular expression (IEEE 1800-2009 15.4.3), and an
associative or unpacked array is not singular (6.4); industry-standard
behaviour rejects both as messages. llg's aggregate messages (records,
queues, dynamic arrays and pattern-built unpacked arrays) are an extension,
and these two types stay outside it: the runtime message value has no nested
associative form, a whole fixed-array source is rejected by a guard although
the native transfer could copy it, and a whole fixed-array destination has no
transfer out of a message value. The aggregate copy-in and copy-out of the
expression forms run before a statement that evaluates the call once, or in
place at each evaluation of a loop condition or step and of an operand that
may be skipped; other expression contexts have neither placement.

### Intended direction

None for the two message types (they follow industry-standard behaviour). The
remaining expression contexts can use the in-place placement once a design
needs them.

### Reproduce

`tests/fixtures/sim/feature_completion/sim_017/neg_assoc_message.sv`,
`neg_array_variable.sv`, `neg_array_variable_get.sv` and
`neg_case_expression.sv`.

## Rejected process-handle forms

**Status:** open (SIM-015).

### Symptom

These legal forms are rejected with explicit diagnostics:

- a `ref` process formal whose actual is a class property or a record
  member (elements of fixed arrays and queues are served);
- an output or inout process formal bound to a process variable in a
  function call inside an expression;
- `try_get`/`try_peek` of a mailbox into a process variable;
- `status()` inside a `wait` condition, an event expression, an `@*`
  sensitivity body or a continuous assignment;
- `suspend()` inside a function body.

An `always_comb` body that reads `status()` runs: its sensitivity holds
variables and selects (9.2.2.2.1), so a status change does not re-evaluate
it, as with industry-standard behaviour. `await()` is a task, so a function
may call it only in a `fork`-`join_none` branch (13.4.4), where it runs.

### Cause

A `ref` process formal aliases counted process-variable storage, so the
callee retains and releases through it; plain handle storage holds pinned
identities without counts and cannot share that alias. Outputs and mailbox
receives bound to a counted variable are copied back by a statement after
the operation, which an expression does not have. Status changes publish no
change event, so a status wait would never wake. Functions cannot suspend,
and the target of `suspend()` is known only at run time.

### Intended direction

Pass the slot kind with the `ref` binding (or route plain slots through a
counted temporary with write-through); stage expression outputs through the
caller's statement like other native outputs; publish a status change event
at a safe scheduler point for status waits; allow `suspend()` of another
process from a function with a run-time check for self-suspension.

### Reproduce

`task automatic t(ref process p); ... t(obj.p);`, `x = f(q) + 1;` with
`function int f(output process p)`, `mb.try_get(q)` for a `process q`,
`wait (p.status() == process::FINISHED);`. Fixtures in
`tests/fixtures/sim/feature_completion/ki_audit/`:
`neg_process_ref_property.sv`, `neg_process_ref_member.sv`,
`neg_process_output_expression.sv`, `neg_process_try_get.sv`,
`neg_process_status_wait.sv`, `neg_process_status_event.sv`,
`neg_process_status_implicit.sv`, `neg_process_status_continuous.sv` and
`neg_process_suspend_function.sv`; the served forms are in
`process_handle_forms.sv`.

## Real references and real-array expressions outside stable storage

**Status:** open; SIM-005 implements real/shortreal references to stable
storage and real fixed arrays across subroutine boundaries.

### Symptom

These legal forms reject with explicit diagnostics: a real-array call
operand, equality or conditional arm that is a function call inside an
expression other than a whole-array assignment or another real-array input
(assign it to a variable first); a conditional real-array arm or row selector
that is not a plain variable read or constant; and element-wise real-array expressions
above 4,096 elements (`REAL_ARRAY_ELEMENTWISE_LIMIT`). Whole-array storage
operands of any size pass by block copy.

### Cause

Real-array values that are not storage are lowered to one numeric
expression per element, evaluated where they are used, so operands that would
need a snapshot statement inside an expression are rejected rather than
evaluated twice.

### Intended direction

A real-array value descriptor would let expression operands and larger
element-wise expressions run as cell loops.

### Reproduce

`x = (f() == g()) ? 1 : 0;` with `f` and `g` returning real arrays.
Fixtures in `tests/fixtures/sim/feature_completion/ki_audit/`:
`neg_real_array_call_equality.sv`, `neg_real_array_call_arm.sv` and
`neg_real_array_elementwise_limit.sv`.

## Streaming `with` targets outside the direct assignment path

**Status:** open; RTL-015 represents runtime and partly out-of-bounds fixed
`with` targets in a direct (blocking or nonblocking) streaming assignment, and
RTL-103 adds output copy-out from task and void-function call statements.

### Symptom

A runtime or out-of-bounds `with` range rejects with a located diagnostic in
two places: an output copy-out from a function call inside an expression, and
the target of an intra-assignment-delayed assignment. A constant in-bounds range
works there. Three further forms reject by owner policy (see Cause).

### Cause

Such a range needs its bounds checked and its in-range elements written at run
time (IEEE 1800-2009 §11.4.14.4), which the `StreamAssign` statement does. A
copy-out runs it after the call with the selectors fixed when the call starts.
A function call inside an expression has no statement after it to run it in,
and an intra-assignment-delayed assignment lowers its target as a static
lvalue (SIM-014), so a runtime or out-of-bounds `with` range there rejects; a
constant in-bounds range works.

Three forms reject by owner policy rather than cost: a selector that reads a
target unpacked earlier by the same nonblocking unpack (nothing is published at
issue), by a right-to-left unpack (the consumed width must be known before the
bits are reordered) or by a copy-out (its selectors are fixed before the call).
Assign the length first in its own statement.

### Intended direction

Supporting the expression call needs a writeback form of the checked unpack in
the call ABI. The delayed target needs the `StreamAssign` range check at the
delayed write. The three policy forms stay rejected.

### Reproduce

`tests/fixtures/sim/feature_completion/rtl_103/neg_expression_copyout.sv` and
`neg_copyout_dependence.sv`;
`tests/fixtures/sim/feature_completion/rtl_015/neg_delayed_with_range.sv`,
`neg_nba_with_dependence.sv` and `neg_reversed_with_dependence.sv`.

## Runtime-selected module reference connections have no qualified binding oracle

**Status:** open by policy. Static selected connections and nested packed
projections execute.

### Symptom

A `ref` port connection with a runtime selector, such as
`child c(values[index]);`, rejects with an explicit runtime-reference-rewiring
diagnostic; it must not silently reread `index` on each access or invent a
canonical storage cell.

### Cause

The fixed module-ref path requires constant actual selectors. IEEE 1800-2009
§23.3.3.2 describes hierarchical reference binding, but the retained
runtime-selector characterization has no adjudicated binding/rebinding oracle.

### Intended direction

Qualify that boundary before enabling runtime-selected connections.

### Reproduce

`tests/fixtures/sim/feature_completion/rtl_009/neg_runtime_ref.sv` and
`tests/fixtures/sim/feature_completion/rtl_003/runtime_ref_port.sv`.

## Delayed native continuous writes

**Status:** open; SIM-004 queues untimed and `#delay` nonblocking writes and
drives zero-delay continuous assignments of strings and string records.
SIM-014 executes event and repeated-event intra-assignment timing on string
and handle targets (`s <= @(e) t;`, `s = repeat (2) @(e) t;`).

### Symptom

These legal forms reject with explicit diagnostics: a delayed continuous
assignment to a string or string record (`assign #1 s = t;`, SV 10.3.3), and
a blocking `#delay` assignment of a record with a string member and a
conditional source (`r = #1 (c ? x : y);`).

### Cause

A delayed continuous driver keeps an inertial pending value per driver, which
holds a packed (`sv4_t`) payload only and owns no string.

### Intended direction

Give the inertial driver an owned native payload (string or chandle) next to
the packed one, reusing the `llg_nba_t` native member layout and its destroy
path.

### Reproduce

`tests/fixtures/sim/feature_completion/sim_004/neg_delayed_string_continuous.sv`
and `tests/fixtures/sim/feature_completion/ki_audit/neg_delayed_native_record_conditional.sv`.

## Class-keyed associative arrays have explicit gaps

**Status:** open. The forms below reject with a located diagnostic.

### Symptom

- An index of a built-in class (`semaphore`, `mailbox`, `process`), or a nested
  key of a type with no runtime form, rejects at the declaration.
- `find_index`, `find_first_index`, `find_last_index` and `unique_index` of a
  class-keyed array, and `item.index` in a `with` clause over one.
- A VPI dynamic-array view of a class-keyed array.
- `a[f()] += 1`, a compound write whose key calls a function, as for any key.

### Cause

The locator index forms need a queue of handles as their result, which the
packed locator path does not produce. Key order reads
`llg_gc_header_t.serial`, which only collected class objects carry.

### Intended direction

Give locator results a handle queue form.

### Reproduce

`tests/fixtures/sim/feature_completion/sim_006/neg_class_key_builtin.sv`,
`neg_nested_key_type.sv` and `neg_class_key_item_index.sv`;
`tests/fixtures/sim/feature_completion/ki_audit/neg_class_key_find_index.sv`
and `neg_class_key_call_compound.sv`.

## Selects of properties of computed class handles

**Status:** narrowed (FIN-001 G3, F07-09 and F07-14). Whole properties of a
computed handle (`get().v`, `q.pop_front().s`, `get().nxt.v`) read, write
and mutate with the handle evaluated once per operation, a packed select of
one reads (`get().b[3:0]`), and elements of an unpacked array property
(`get().a[1]`, `q.pop_front().a[0]`) read, write and take compound
assignments
([`computed_handle_element.sv`](../tests/fixtures/sim/feature_completion/ki_audit/computed_handle_element.sv)).

### Symptom

A packed select of such a property as a write target (`get().b[2] = 1'b1;`,
`get().b[1:0] += 1;`) stops compilation with "cannot resolve array base of
select" (a compound write: "cannot resolve base signal of select"), located
at the statement.

### Cause

Packed selects on class properties as write targets are resolved from
declaration paths (`h.b[2]`); the `MemberSelect` node of a computed base
reaches only the whole-property access.

### Intended direction

Resolve the select's base through the same computed-handle property access
(`computed_handle_property`) and build the packed element access on it,
capturing the handle once for read-modify-write forms.

### Reproduce

`tests/fixtures/sim/feature_completion/ki_audit/neg_computed_handle_select_write.sv`
(`get().b[2] = 1'b1;`, expected `h.b == 8'b00000100`).

## Records of the leaf form: two errors a column does not have

**Status:** open (FIN-001 rule 7: records below `LLG_RECORD_COLUMN_MIN_CELLS`
cells use the leaf form).

### Symptom

Below 16 cells, two forms that a column accepts stop compilation. A constant
select beyond a `string` or handle member array (`r.s[20] = "x"` with
`string s[8]`) reports "cannot resolve array base of select", where SV 7.4.6
reads the default and writes nothing. A whole record with a class handle member
assigned to a member element (`r.a[5] = x` with `e_t a[8]`) reports "assignment
between incompatible unpacked aggregate types".

### Cause

The leaf form names one object per cell, so an index outside the array names
none, and `equivalent_copy_shape` accepts only `chandle` and `void` handles.

### Intended direction

Give the leaf form what the column does: an out-of-range constant select that
reads the default and drops the write, and a whole-value copy of class handle
members.

### Reproduce

`tests/fixtures/sim/feature_completion/ki_audit/neg_leaf_string_select.sv` and
`neg_leaf_handle_member_element.sv`.

## Record results with real members as equality operands

**Status:** open (FIN-001 G3, F07-03). Records with real members compare with
`==`, `!=`, `===` and `!==` from variables, members and subroutine formals
(decision R05-D3 for `===`).

### Symptom

A function result of a record type with a real member used directly as an
equality operand (`mk(2.5) == a`, `mk(2.5) === a`) stops compilation with
"real aggregate bit-stream source is not supported".

### Cause

Record-valued call operands of equality are flattened into one packed value,
which has no representation for a real leaf.

### Intended direction

Compare a record call result through a record temporary, member by member,
as variable operands are compared.

### Reproduce

```systemverilog
module tb;
  typedef struct { int i; real r; } rec_t;
  rec_t a = '{1, 2.5};
  function automatic rec_t mk(real r);
    mk.i = 1;
    mk.r = r;
  endfunction
  initial begin
    $display("%0d %0d", mk(2.5) == a, mk(2.5) === a);  // expected 1 1
    $finish;
  end
endmodule
```

`tests/fixtures/sim/feature_completion/ki_audit/neg_record_real_call_equality.sv`
holds the `==` form.

## `%p` of tagged unions and built-in synchronization handles

**Status:** open. SIM-024 formats every other value under `%p` from a
display-type table.

### Symptom

Two forms reject during lowering instead of printing: tagged unions and
`semaphore`/`mailbox` handles held directly in a `%p` argument. As class
properties they print as handles.

### Cause

The display-type table has no tag view (SV §21.2.1.7 leaves the text to the
implementation, which would print the active member), and semaphore and
mailbox objects are runtime records, not class layouts.

### Intended direction

A tagged-union table entry that reads the tag and active member, and handle
kinds for the two runtime records.

### Reproduce

[`neg_tagged_union_pattern.sv`](../tests/fixtures/sim/feature_completion/sim_024/neg_tagged_union_pattern.sv)
and
[`neg_pattern_semaphore.sv`](../tests/fixtures/sim/feature_completion/ki_audit/neg_pattern_semaphore.sv)
(`semaphore s = new; initial $display("%p", s);`).

## Deferred reports that name class state or activation storage

**Status:** open. `$strobe`, `$monitor` and the file variants re-evaluate their
arguments in the Postponed region. Class properties, reports in class methods
and static string and handle variables work ([S25-D9](lrm_decisions.md),
[sim_025](../tests/fixtures/sim/feature_completion/sim_025/readme.md)).

### Symptom

A `$monitor` of a member of an unpacked structure property or of a static
subroutine record follows the record's contents marker
([`monitor_record_property.sv`](../tests/fixtures/sim/feature_completion/sim_025/monitor_record_property.sv)):
it evaluates its list after a store into any member of that record and prints
when a listed value changed. What is left:

- **`$monitor` through a computed handle** (`f().v`) rejects before C
  compilation with a "native or impure calls in evaluator callbacks" error,
  located at the procedure that issues it: nothing can name the storage without calling the function
  again.
- **Some reads are not exact**: such a `$monitor` re-evaluates at every settled
  slot, which costs one evaluation per slot. These are a virtual-interface member,
  `%p` of an object, a mailbox or semaphore method other than a callee's
  `num()`, and a call whose callee reads class storage through a handle that
  is not bound: a callee local that holds different handles on different
  paths ([`monitor_method_every_slot.sv`](../tests/fixtures/sim/feature_completion/sim_025/monitor_method_every_slot.sv)),
  an implicit `this` in a report issued inside a method, a receiver or actual
  other than a static handle variable or a handle property chain from one
  (`arr[0].get()`), or a virtual-interface member read inside the callee.
  Calls of functions and nonvirtual methods whose reads bind are exact
  ([`monitor_method_calls.sv`](../tests/fixtures/sim/feature_completion/sim_025/monitor_method_calls.sv)).
- **A virtual method call, or a statement-bodied function with a class-handle
  local in a list with no argument that has private effects**, rejects before C
  compilation with a "side-effect-capable evaluator expressions" error,
  located at the procedure that issues it: the read-only Postponed evaluator does not dispatch virtual methods or
  run such a body yet
  ([`neg_monitor_virtual_method.sv`](../tests/fixtures/sim/feature_completion/sim_025/neg_monitor_virtual_method.sv),
  [`neg_monitor_handle_local_helper.sv`](../tests/fixtures/sim/feature_completion/sim_025/neg_monitor_handle_local_helper.sv)).

A report reads container and string actuals at the end of the slot, and builds
a conversion or an unpacked concatenation then ([S25-D8](lrm_decisions.md)). A
neighbouring form still rejects at the call:

- **A `$strobe` actual with a side effect that needs building**
  ([`neg_strobe_container_conversion.sv`](../tests/fixtures/sim/feature_completion/sim_025/neg_strobe_container_conversion.sv)):
  the Postponed region writes nothing.
- **A `$strobe` helper that prints** (`$display` inside the function): it has
  output effects, which the private evaluator refuses with the
  "side-effect-capable evaluator expressions" error, located at the procedure
  that issues it; about a day.

`$finish` also drops the reports still pending in its own slot
([S25-D5](lrm_decisions.md)); a `$strobe` in a `final` block rejects because no
Postponed region runs after finals.

### Cause

The Postponed evaluator is private and read-only: it cannot call a function
again to name storage behind a computed handle, does not dispatch virtual
methods, refuses output and write effects, and has no region after `final`
blocks. A `$monitor` read it cannot bind to a change marker is evaluated at
every settled slot.

### Intended direction

Bind more reads to markers, dispatch virtual methods and run statement bodies
in the private evaluator, and allow helper output effects (about a day).

### Reproduce

[`neg_monitor_virtual_method.sv`](../tests/fixtures/sim/feature_completion/sim_025/neg_monitor_virtual_method.sv),
[`neg_monitor_handle_local_helper.sv`](../tests/fixtures/sim/feature_completion/sim_025/neg_monitor_handle_local_helper.sv),
[`neg_strobe_container_conversion.sv`](../tests/fixtures/sim/feature_completion/sim_025/neg_strobe_container_conversion.sv);
`tests/fixtures/sim/feature_completion/ki_audit/neg_monitor_computed_handle.sv`,
`neg_strobe_printing_helper.sv` and `neg_strobe_in_final.sv`.

## Effectful helpers in runtime-callback evaluators

**Status:** narrowed by RTL-007b; two list forms remain.

Legal helpers with visible writes, persistent static state or descriptor-array
formals now run in processes for blocking, intra-assignment, nonblocking and
`->>` event controls, force sources and named-event/real event lists, and in a
private Postponed evaluation for `$monitor`/`$strobe` (writes outside the
helper reject there by SV 4.4.2.9).

### Symptom

Still rejected: a named event in the same list as such a helper whose
sensitivity includes unpacked-array, container or string storage, and event
handles other than declared named events in such a list or as the target of
such a `->>`. Stores a `$monitor`/`$strobe` helper makes to its own static
storage publish no event, so a hierarchical wait on that storage does not
wake.

### Cause

The atomic mixed wait has no source form for unpacked-array, container or
string markers, and the helper process names only declared named events. The
private Postponed evaluation publishes no change events.

### Intended direction

Give the mixed wait source forms for those markers and event-handle operands;
publish the private evaluation's static stores.

### Reproduce

[`neg_named_event_array.sv`](../tests/fixtures/sim/feature_completion/rtl_007b/neg_named_event_array.sv)
(a named event beside a helper that reads array storage) and
`tests/fixtures/sim/feature_completion/ki_audit/neg_helper_event_handle.sv`
(an event handle other than a declared named event in a helper's list).

## Waited `ref` formals with unobservable actuals

**Status:** narrowed by SIM-013, SIM-009-A03 (the call-site expansion is
gone; every remaining form rejects at the call with its location) and FIN-001
R2.

### Symptom

These legal forms reject explicitly instead of running:

- A task waiting on a member of a native record `ref` formal (an unpacked
  structure with a string, container or class member) called with a whole
  record that is a class property or a task-automatic variable
  ("waits on a native
  record `ref` formal, so each of its record actuals must be a module, static
  or procedural-block record"). In the column form (16 or more cells), also
  one called with an element of a fixed array, queue, dynamic array or class
  property, directly or through a virtual interface ("of the column form ...
  publishes no change that form follows"); the per-cell form serves elements.
- A task waiting on a `ref real` formal bound to a real member of such a
  record property (`h.r.rv`; "storage of record class property ... cannot be
  observed").
- `@(s)` on a subroutine string or handle that is neither a `ref` formal nor
  shared with a fork branch (only the waiting process could change it).

Everything else takes the typed call path: packed and real `ref` formals
bound to variables, fixed-array, queue, dynamic and associative array
elements, unpacked-structure members and class properties (also elements of
container class properties, and members of record properties whose object
marker a packed `ref` formal then follows), unpacked
aggregate and virtual-interface `ref` formals, and string and handle `ref`
formals bound to module, static, process or fork-shared variables, class
properties and array elements. Handle stores through a `ref` formal publish at
the store.

### Cause

A native record `ref` formal's body reads leaves that only a record
specialization binds; class-property and automatic records have no static
leaves. A column-form formal takes an element holding native members by
address, with no marker beside it.

### Intended direction

Pass a native record's leaf markers with the record value.

### Reproduce

`tests/fixtures/sim/feature_completion/sim_009/neg_record_ref_wait_actual.sv`,
`neg_string_event_control.sv`;
`sim_014/neg_ref_wait_record_property_whole.sv` and
`neg_ref_wait_record_property_real.sv`;
`sim_008/record_ref_interface_element_waits.sv` with `-D N=16`.

## Level waits on the identity of an event variable reject

**Status:** rejected with a located diagnostic (no longer a hang). Class event
properties are served.

### Symptom

`wait (k == g)` on event variables or a `ref` event formal, and an event
control on the same comparison (`@(k == g)`), stop compilation with "an event
variable comparison cannot be observed by a wait", located at the comparison.

### Cause

Event variables have no change marker, so a wait on their identity could never
resume after `k = g`.

### Intended direction

Give `llg_event_assign` a marker if a design needs the form served. Some
simulators reject any event operand in a `wait` expression. Estimate: half a
day to serve.

### Reproduce

`tests/fixtures/sim/feature_completion/sim_014/reject_event_wait_compare.sv`,
`reject_event_ref_wait_compare.sv` and `reject_event_control_compare.sv`
(`sim_014::event_variable_comparisons_in_waits_reject`).

## Wired-logic nets in bidirectional switch networks

**Status:** open (FIN-001 G3, F08-11; ADV-005). Wired nets in a network of
one wired kind without pull or supply nets work (decision A5-D7,
`adv_005/wired`).

### Symptom

A switch network that joins a wired-AND (`wand`, `triand`) and a wired-OR
(`wor`, `trior`) net, or a wired net and a `tri0`, `tri1`, `supply0` or
`supply1` net, is legal but stops with "its switch network joins ...; such a
mixed network is not simulated".

### Cause

The solver takes the dominant value of a wired group (0 or 1) first among
equal strengths, which needs one wired kind per network. How a wired net
combines with a pull or supply net across a switch is not settled: the text
is silent and the industry-standard behaviour observed is irregular (a
`tran`-joined `trior` and `tri0` resolve as a plain wire without the pull).

### Intended direction

Per-group dominance order for mixed networks (a second deferral pass) and a
decision case for the pull and supply combinations. Estimate: 1 day.

### Reproduce

`tests/fixtures/sim/feature_completion/adv_005/neg_wired.sv`,
`neg_wired_supply.sv`.

## Set membership items outside packed storage

**Status:** open (FIN-001 G3, F07-11). Class-handle and chandle selectors
execute (`sim_019/handle_inside`, decision R14-D1).

### Symptom

These legal `inside` set items reject with explicit diagnostics: a queue,
dynamic or associative array of strings (`s inside {sq}`); a container of
reals, handles or records (`r inside {rq}`); an element of a container of
containers (`x inside {qq[0]}`, "container element access ... requires a
scalar value"); and a container-result call (`x inside {g()}`).

### Cause

String set items traverse only fixed and aggregate string storage; the
container item form (`IrInsideItem::Container`) compares packed elements.
Nested container elements and container results have no addressed-container
operand (see
[resizable containers](#resizable-containers-at-subroutine-object-and-nesting-boundaries)).

### Intended direction

A string container item that walks the container's string elements, a real
comparison for real containers, and the addressed-container operand for the
nested and call forms. Estimate: 1 day for string and real containers.

### Reproduce

`tests/fixtures/sim/feature_completion/sim_019/neg_string_container_inside.sv`
and `neg_real_container_inside.sv`;
`tests/fixtures/sim/feature_completion/ki_audit/neg_inside_nested_container.sv`
and `neg_inside_container_call.sv` (the call form reports "container result
of `g` ... must be assigned whole to a container variable").

## Waits on storage without a change notification

**Status:** narrowed (fin-gaps-e). Per-object container properties, mailbox
`num()`, callee handle locals with one binding and virtual-interface members
read in called functions now wake waits
(`ki_lrm_audit_b/observed_storage`, `callee_dataflow`).

### Symptom

A `wait` condition or event expression that reads storage publishing no
change is still rejected at compile time with its source location ("cannot
be observed by a wait or event expression"), because the wait could never
resume: semaphore methods and mailbox methods other than `num()`, class
properties a called function reads through a local that holds different
handles on different paths or through any other expression (a container
element, a call result), and virtual methods (industry-standard behaviour
rejects these in `wait` conditions as well).

The same rejection applies to a property selected through a call result,
`wait (f().x == 1)`, and to a member of an automatic record with a native member
(a string, a real, a handle, a container) that a fork shares and that something
stores into inside a fork branch, or anywhere once a `join_none` or `join_any`
fork names it. An automatic record that nothing can store into while a branch
waits is read as it is, and its container members have their own markers.

### Cause

The rejected reads publish no change: a semaphore or mailbox method result, a
property behind a handle the waiter cannot bind, a virtual method's result, a
call result, and a forked automatic record, which is one value per activation
with no change marker.

A member of an unpacked structure with a native member that is a class property
or a static subroutine variable is waited on through the record's contents
marker ([S07-D18](lrm_decisions.md)): `wait (h.r.a == 1)`, `@(h.r.a)`,
`@(posedge h.r.a)`, `$monitor` and `always @*` evaluate what they read again
after a store into any member and resume only when its value changed. The cost
is that evaluation: a waiter on `h.r.a` runs its comparison when `h.r.s`
changes, and, for a class property, when a handle or container property of the
same object changes (they share the object's marker). A change undone before
the writer suspends wakes nobody, a string member's included
([T1-D11](lrm_decisions.md)). A run-time index into a member array of a record
property (`h.r.arr[k]`) or of a static subroutine record is served, also in a
`wait` condition.

### Intended direction

Bind callee locals to every handle they can hold (a dependency set instead of
one receiver).

### Reproduce

`tests/fixtures/sim/feature_completion/ki_lrm_audit_b/neg_*.sv`;
`tests/fixtures/sim/feature_completion/ki_audit/neg_wait_container_element_property.sv`
(a callee reading a property through a container element);
`tests/fixtures/sim/feature_completion/sim_014/reject_wait_call_result.sv`,
`reject_wait_automatic_record.sv`, `reject_event_automatic_record.sv`.

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

## Driven net-array cells emit per-cell electrical code

**Status:** open for driven cells. RTL-010 removed the quadratic net-array
driver discovery; KI-NET-INTERVAL removed all per-cell cost of undriven cells.

### Symptom

Driven cells still cost about 2.4 KB of C each: every driven cell owns an
electrical group, a declared-view alias row and an observation cell, and a
whole-array continuous driver gathers its RHS cells into one packed value with
one element read per cell (19.8 MB for an 8,192-cell
`wire [7:0] n[8192]; assign n = src;`). Lowering is linear in the cell count:
driver sources are indexed by the cells they drive once.

Input-port links into a net-array formal and `unconnected_drive` pulls on one
(RTL-012) gather the cells into one packed contribution the same way. Above
the packed width limit they keep the older per-element storage write, which
bypasses the formal's internal drivers.

### Cause

A driven cell keeps its electrical group, declared-view alias row and
observation cell, and whole-array drivers gather their sources into one packed
value. Undriven cells already avoid all of it:

Lowering first classifies net-array cells by interval
([`collection/net_cells.rs`](../src/sim/codegen/lowering/collection/net_cells.rs)).
A cell that no structural driver, true alias, selected inout connection or
force/release target reaches, together with every whole-array inout peer of
it, is undriven: its value is the constant of its effective (collapsed) net
type, so it becomes part of a typed `IrNetCellRun` and gets no electrical
group, declared view, bit-level alias/type-plan entry or generated C. Only one
representative class per connection shape enters the port type plan. A `%v`
read of an undriven cell materializes just that cell's group. The runtime
indexes dependency bindings by target, so dense net arrays no longer make
startup binding quadratic or every signal write linear in the cell count.

Measured on a quick build (`--gen-only`; whole-array inout with one driver on
each side, and one UDP output bit in a `wire [7:0]` array):

| Fixture | Before | After |
| --- | ---: | ---: |
| inout, 4,096 cells | 2.1 s / 8.7 MB | 0.06 s / 31 KB |
| inout, 65,537 cells | 28.0 s / 140.5 MB | 0.57 s / 31 KB (37 MB RSS) |
| UDP, 4,096 cells | 1.1 s / 6.6 MB | 0.04 s / 21 KB |
| UDP, 200,000 cells | > 500 s / 328 MB | 0.36 s / 21 KB (39 MB RSS) |

The 200,000-cell model then ran in 217 s before the dependency index and
0.14 s after. [`sim_net_interval`](../tests/sim_net_interval.rs) caps both
large fixtures' `model.c` at 1 MiB and codegen at 20 s per optimizer mode.

### Intended direction

For driven cells: a descriptor-backed net-array cell table with a loop over a
contiguous RHS view, keeping per-cell resolution state but not per-cell
generated code; see the remaining steps in "Declared net views retain per-bit
lowering bindings".

### Reproduce

`wire [7:0] n[8192]; assign n = src;` with `--gen-only`, then measure
`model.c`; [`sim_net_interval`](../tests/sim_net_interval.rs) holds the
undriven caps.

## Unpacked records with dense member arrays emit per-cell code

**Status:** narrowed; found by codegen-size. A member array of at least
`LLG_RECORD_COLUMN_MIN_CELLS` (16) cells is stored as a column
([record storage by size](sim_performance_tradeoffs.md#record-storage-by-size)),
so its selections do not grow with the array. Below 16 cells each cell is its
own leaf, a structural rule: the code is small and a reader needs no lookup.

### Symptom

Every shape follows that rule. A member array of integral, packed or `real`
elements in a structure that also holds a member with no packed width, such as
`typedef struct { logic [3:0] a; real m[4096]; string t; } rec_t;`, is one
column; `string`, handle and record elements, and any element above 4,096 cells
without a packed width, are one container. No operation on such a member is
linear in generated C.

Two shapes are not columns at this size:

- An unpacked union with a member array keeps its packed image.
- A record with a member array above 4,096 cells and another member array whose
  elements have no packed width, such as `struct { int p[5000]; string d[20]; }`,
  keeps the packed member in a container too: 40 bytes per declared cell instead
  of the sparse column's cost per written cell.

A waveform dump omits the members held in a container (`string`, handles,
records with such members)
([waveform dump coverage limits](#waveform-dump-coverage-limits-sim-047)).

A use the column form cannot lower is an error at its source location. These
limits apply to the form named:

- both forms: a row of a multidimensional member array of a queue element at a
  run-time index (`q[0].a[k] = '{1, 2}`); assign through a record variable
  first. A whole member array of an element written through an event control
  (`m.e[1].a <= @(e) v`).
- per-cell form, below 16 cells: a member array selected on a call result
  (`f().a[1]`); the column form serves it.
- column form: a task or virtual-interface method that waits on a record `ref`
  formal bound to an element holding native members rejects
  ([waited `ref` formals](#waited-ref-formals-with-unobservable-actuals)).
- per-cell form, below 16 cells: a whole record of packed members assigned to
  or read from an element of a member array at a run-time index
  (`r.rows[i] = x`; a record with a string member works). A record of packed
  members only, read whole from a container element into a fixed-array
  element, a class property, an output formal or an initializer
  (`fa[2] = q[0]`). The column form has none of these limits.
- per-cell form, below 16 cells: a record element selected at run time as a `ref`
  actual lowers the call once per element, so only statement calls can take it and
  a call site has at most 256 branches (`neg_record_ref_element_select_value`,
  `neg_record_ref_element_select_product`). The column form has neither limit.
  A task waiting on the formal stops the run when the index selects no element,
  in both forms, and so does one waiting on any `ref` formal bound to an
  element of a fixed array, queue or dynamic array whose index selects none
  (`ref_wait_unselected_*`). A forwarded real `ref` formal carries no
  descriptor to test: the call that first binds it stops. A callee that does not
  wait on the formal reads the default of the element type and drops writes,
  where some simulators stop at any such binding (decision S13-D6).
- column form: an event control on a member of a `ref` formal whose actual is an
  automatic record of a subroutine; a module, static or procedural-block record
  works, and so does an element of a column.

### Cause

Each storage form lowers whole values and selections its own way; a use that
the form at hand cannot lower is an error rather than a per-cell expansion.
codegen-size made whole-value reads and writes of unpacked arrays O(1)
in generated C (`emit_c/owned/dense_cells.rs`), and removed the lowering
stack overflow that whole equality of such records hit from about 256 leaves
(the leaf comparisons are now combined as a balanced tree).

### Intended direction

Lower each listed use in the form named, as the other form already does where
it has none of the limit; give the two non-column shapes a column.

### Reproduce

The record above, `rec_t a, b; initial b = a;` and `--gen-only`, then measure
`model.c`. The located limits:
`tests/fixtures/sim/feature_completion/sim_008/neg_record_ref_element_select_value.sv`
and `neg_record_ref_element_select_product.sv`; in
`tests/fixtures/sim/feature_completion/ki_audit/`:
`neg_record_call_member_select.sv`, `neg_record_queue_row_select.sv`,
`neg_record_member_array_event_write.sv`, `neg_record_rows_runtime_index.sv`,
`neg_record_container_element_to_fixed.sv` and
`neg_record_ref_member_wait_automatic.sv`. Forms formerly listed here that
run: `record_member_forms.sv`.

## Gate and UDP array elements emit one process function each

**Status:** open; found by RTL-020.

### Symptom

A 4,096-element mux UDP array produces 9.8 MB of `model.c`
(about 2.4 KB per element) after RTL-020's dense-index lookup, which already
removed the per-evaluation input copies (15.8 MB before). Evaluation cost per
element is constant; the cost is C size and compile time, linear in the
element count. Sequential UDP arrays (ADV-003) follow the same pattern: each
element also owns one small state record, while the next-state table and the
step function are shared per definition.

### Cause

Each element of a gate or UDP instance array is lowered to its own
continuous process, and each process gets its own C function because the
elements read and write different constant bits, so exact body sharing does
not merge them.

### Intended direction

One shared process body per array declaration, parameterized by the element's
bit offsets, like the table-driven net-contribution batches.

### Reproduce

`mux3 m[4095:0] (y, sel, a, b);` over 4,096-bit vectors and `--gen-only`, then
measure `model.c`.

## Unsupported force and release targets

**Status:** narrowed (K3). Whole unpacked net arrays are forced and
released (`sim_023/whole_net_arrays`); string, class-handle and event
variables are rejected by decision S23-D5 (industry-standard behaviour). In
SystemVerilog, elements of static unpacked array variables and members of
their unpacked-structure elements are forced and released (decision S23-D6),
including lazily allocated cells and the member arrays of records in both
record storage forms.

### Symptom

This legal target stops the run with a located error instead of being
forced: a member of an element of an array of records that have a real,
string, class-handle, chandle, event or container member (for example
`force x[1].a = 1;` on `struct { int a; real r; } x[2]`).

### Cause

Such a record is a
native value: a whole-element or whole-array write rebuilds its members, so a
member has no fixed address the force runtime can overlay, and its waiters
watch the array's change marker rather than the member.

### Intended direction

Supporting it needs a force target that names the array, the element and the member path, re-found
after every write of the element, and those native write paths (element and
whole-array copies, located nonblocking writes, member setters) keeping the
forced member and publishing the array's marker after a force or release.

### Reproduce

`native_record_member_is_a_known_limitation` in
[sim_023](../tests/fixtures/sim/feature_completion/sim_023/readme.md).

## Operator-overload update values on oversized or native targets

**Status:** RTL-017, RTL-104 and SIM-021 execute operator overloads (SV
§11.11), including increment values, once-evaluated targets, relational
expected types, package overloads, and native-record and oversized update
values on the right-hand side of an assignment. Three forms are left.

### Symptom

These report "an overloaded operator update on a target above the
1048575-bit packed value limit or with native members yields a value only as
the right-hand side of an assignment, needs side-effect-free target
selectors, and has a postfix value only for native records" when the target
is a record with a string, real, handle or container member, or a fixed value
wider than the packed value limit (a 65,537-element `int` array):

- a target selector with side effects (`a[next()]++;`, `a[next()] += b;`);
- an update used as a value anywhere other than the right-hand side of an
  assignment (`show(x++)`, `while (x++ < lim)`);
- the postfix value of an oversized target (`y = big++;`).

Statement forms with side-effect-free targets (`x++;`, `x += b;`, `for`
steps) and `y = x++;`, `y = ++x;`, `y = (x += b);` on native records (and the
prefix and compound values on oversized targets) run.

### Cause

Packed targets lower to the `Mutation` expression, which captures the
target's current value once. Native records and descriptor-backed fixed
values have no such capture: SIM-021 runs their assignment-value forms as
`A = f(A, B)` with the target read again (and a native temporary for a
postfix value), which needs side-effect-free selectors and a statement
context.

### Intended direction

Give `IrFixedValue` and native record roots a once-resolved read/modify/write
form whose old value can be kept, usable inside expressions.

### Reproduce

`tests/fixtures/sim/feature_completion/rtl_104/limit_native_value.sv` and
`tests/fixtures/sim/feature_completion/sim_021/limit_native_value_argument.sv`.

## Strict 2001 profile checks grammar and listed semantics

**Status:** open by design; the grammar side is closed.

### Symptom

A 2001 constraint that is neither grammar nor one of the listed semantic rules
stays admitted under `--edition v2001` unless the frontend rejects it, for
example an `output real` module port. Navigation snapshots have no subroutine
bodies, so the body rules do not run there. Directives in trivia are checked
by token, not by the syntax profile.

### Cause

The pinned frontend parses 1364-2001 sources with its 1364-2005/SystemVerilog
grammar. The wrapper's `Verilog2001SyntaxProfile` closes the grammar side: a
Verilog-2001 compile rejects every parsed syntax kind outside the IEEE
1364-2001 Annex A allowlist and the listed optional parts of admitted kinds,
on the macro-expanded trees. [`editions.rs`](../src/core/compile/editions.rs)
adds keywords, system names and the semantic rules in the
[edition table](sim_features.md#target-language-editions), including variable
drivers. Other 2001 constraints have no rule.

### Intended direction

Add a rule to the edition table for each further 2001 constraint a design
needs; the profile does not aim at a complete 1364-2001 semantic check.

### Reproduce

Compile `module tb(output real r); initial r = 1.0; endmodule` with
`--edition v2001`: it is admitted.

## Deferred assertion actions with container, event or virtual-interface arguments

**Status:** open (SIM-036 deferral; kept by FIN-001 A3: uncommon forms).

### Symptom

A deferred assertion action (SV 16.4) whose call passes a queue, dynamic or
associative array, or named event by value, or that calls a task or function
through a virtual interface handle, rejects with "unpacked aggregate,
container and event arguments of deferred immediate assertion actions are not
supported" or "virtual-interface method calls are not supported as deferred
immediate assertion actions", located at the action. The action frame
captures packed, real, string and opaque (chandle/class handle) values
(unpacked structs and fixed arrays already work); a container needs an owned frame object copied at issue time
(`llg_frame_capture_object`) and an event needs its object identity captured
like a method receiver. Fork-branch outermost blocks are also not treated as
outermost procedure scopes for the 16.4.4 flush.

Subroutine calls attached to a sequence match (SV 16.11, SIM-039) use the
same captured frame and share these limits, reported as "sequence match-item
call" diagnostics; a match-item task with a timing control is also rejected.

### Cause

The action frame holds fixed-size captured values; it has no owned copy of a
container, no captured event identity and no virtual-interface receiver.
Industry-standard behaviour accepts these forms; it was seen reading a
queue or dynamic array argument when the action runs rather than when the
assertion is evaluated, which 16.4 (L21223-21225) does not allow.

### Intended direction

Copy a container argument into an owned frame object at issue time
(`llg_frame_capture_object`), capture an event's identity and a virtual
interface handle like a method receiver.

### Reproduce

[`sim_036/neg_queue_action.sv`](../tests/fixtures/sim/feature_completion/sim_036/neg_queue_action.sv),
[`sim_036/neg_vif_action.sv`](../tests/fixtures/sim/feature_completion/sim_036/neg_vif_action.sv)
and
[`sim_039/neg_match_call_queue.sv`](../tests/fixtures/sim/feature_completion/sim_039/neg_match_call_queue.sv);
the timing-control form is `sim_039/neg_match_call_timing.sv`; a dynamic array
and a named event by value:
`tests/fixtures/sim/feature_completion/ki_audit/neg_dynamic_array_action.sv`
and `neg_event_action.sv`; an associative array:
`neg_associative_array_action.sv`.

## Assertion locals in join operands and end points

**Status:** open (SIM-039 deferral; kept by FIN-001 A3).

### Symptom

Property locals (local input formals and declared locals with a declaration
assignment) are initialized in every property form since FIN-001 A2: the
property engine places the initialization at the semantic leading clock of
the property they start (F.4.2, decision A13-D8), so later property operands
(`always`, `until`, `and`/`or` of properties, ...) read them. Local-variable
assignments inside `and`/`intersect`/`throughout`/`within` operands and
local variables passed to sequence end points (`s(v).triggered`, `@(s(v))`)
are still rejected; the runtime keeps one local array per thread and a join
keeps the locals of the operand that matched last. A local read by an
`accept_on`/`reject_on` condition is rejected as well (a frontend error).
The join message is located at the composite's first operand.
The guard "local variable `v` of the property is initialized at the property
attempt's start, which this property form does not support" has no form that
reaches it (`not`, `until` and `if`/`else` with a declared local simulate); it
is kept as a defensive error and has no test.

### Cause

The runtime keeps one local array per sequence thread, and a join keeps the
array of the operand that matched last instead of merging the variables that
flow out of each operand (16.10 d). An end point is evaluated in an attempt
of its own, so no local can flow back from it to the reading attempt.

### Intended direction

Merge the operand threads' locals at join completion by flow (assigned in
one operand: that value; in both: blocked). Industry-standard behaviour
serves the one-operand join form; it does not implement `local output`
sequence formals, so the end-point form has no reference behaviour.

### Reproduce

[`sim_039/neg_join_local.sv`](../tests/fixtures/sim/feature_completion/sim_039/neg_join_local.sv)
and
[`sim_039/neg_endpoint_local_output.sv`](../tests/fixtures/sim/feature_completion/sim_039/neg_endpoint_local_output.sv).

## Implicit sensitivity through handles a process cannot evaluate

**Status:** open (gate-fixes-2 limit, decision S99-D7).

### Symptom

`always_comb`, `always_latch` and `@*` wait on a member read through a
virtual-interface handle by evaluating the handle each time they arm. Three
forms have no handle the arming process can evaluate, so they stop
compilation at the read with "the implicit sensitivity of `<scope>` cannot
follow ...": a member read inside a called function through the function's
handle formal or local (`function f(virtual bus_if b); return b.a;`), a member
read through an element of a virtual-interface array (`va[1].a`), and an
interface method call through a handle under `always_comb` (`v.f()`, whose
body the bound instance selects). Elements of unpacked array members are
followed only through a virtual-interface variable.

### Cause

The arming process evaluates a handle it can name; a callee's formal or local,
an array element handle and a method body selected by the bound instance are
none it can name.

### Intended direction

Supporting the formal needs the call's actual bound at arm time (like
`CalleeReceivers` for class handles); the method call needs the method body's
reads per bound instance.

### Reproduce

`tests/fixtures/sim/feature_completion/sim_099/neg_vif_sensitivity_*.sv`.

## Modport expression ports through virtual interfaces

**Status:** open (RTL-102 deferral).

### Symptom

A modport expression port `.p(expr)` (SV 25.5.4) executes through static
interface ports and hierarchical references, but through a virtual interface
`vif.p` rejects with "modport expression port `p` is not supported through
virtual interface view".

An interface instance used directly as a value outside a virtual-interface
assignment, argument or comparison (the message of `mb.put(i1)` for a
`mailbox #(virtual ifc)`) also rejects ("instance `i1` is used as a value";
assign it to a virtual interface variable first).

### Cause

A virtual interface handle has runtime member slots only for interface
storage.

### Intended direction

Supporting it needs a per-descriptor evaluator (and, for outputs, a writer)
for each expression port, selected by the bound instance at run time.

### Reproduce

`interface i; logic [7:0] a; modport m(input .p(a[3:0]));
endinterface`, `virtual i.m v = inst;` and `$display("%h", v.p);`
(`tests/fixtures/sim/feature_completion/rtl_102/neg_virtual_interface_expression_port.sv`);
`tests/fixtures/sim/feature_completion/sim_012/neg_instance_mailbox_put.sv`
(FIN-001 F12-02).

## Tagged-union members of packed-array elements

**Status:** open (boundary kept when member access through packed-array
elements was admitted).

### Symptom

A member of a tagged union that is an element of a packed array, such as
`tp[i].a` for `t_t [1:0] tp` with `typedef union tagged packed {...} t_t`,
rejects with ``tagged-union member `a` of a packed-array element is not
supported``. Other packed structure and union members of packed-array
elements work, as do tagged-union members of whole signals. Until it is
served, select the element into a tagged-union variable first.

### Cause

Reading or writing it needs the tag check on the selected element; the packed
element-member projection (`collection/packed_elements.rs`) carries no tag
guard, so it rejects rather than read inactive payload bits.

### Intended direction

Lower it as an `IrExprKind::TaggedSelect` / `IrLhs::TaggedSelect` whose steps
are the element chain followed by the guarded member step.

### Reproduce

`typedef union tagged packed { logic [3:0] a, b; } t_t;
t_t [1:0] tp; initial $display("%h", tp[0].a);`
(`tests/fixtures/sim/feature_completion/ki_select_bugs/unsupported_tagged_element_member.sv`).

## Clocking input forms outside the sampled storage model

**Status:** open (SIM-033 boundary, K1).

### Symptom

These legal forms reject with a located message instead of sampling:

- string, class handle, event, unpacked or tagged union, queue, dynamic array
  and associative array clockvars ("string, class handle, event, unpacked or
  tagged union, queue, dynamic array and associative array clockvars are not
  supported"), and unpacked clockvars with such an element or member: the
  text does not exclude them, but industry-standard behaviour rejects them
  (unpacked unions included) and llg follows it
  ([K1-D1](lrm_decisions.md#clocking-inputs-fin-001-k1));
- an unpacked clockvar with a real (or shortreal) element or member ("an
  unpacked clockvar with a real, ... element or member is not supported"): its
  value has no packed image. Industry-standard behaviour serves it;
- an edge input skew with a delay (`input negedge #1 x`), which
  industry-standard behaviour accepts.

An `edge` input skew and an edge skew under an event list also reject; the
first is a syntax error and the second an error in industry-standard
behaviour.

### Cause

Clocking inputs sample into one static storage cell per clockvar, sized from
its packed, real or shortreal type, or holding the whole declaration-order
image of an unpacked fixed-size type.

An unpacked clockvar keeps its sample image current with a synthetic process
that rebuilds the whole image on every write to its source, so a write costs
time linear in the aggregate's size; only designs with such clockvars pay it.

Separately, every sampled clocking source keeps a slot history even when no
positive skew reads it (one heap entry per time step with activity), and each
`#0` (Observed) sample event allocates a copy record plus a region-callback
entry. Reusing a per-source spare entry, skipping the unread history and
reusing one copy record per clockvar halve the allocations, but a pinned A/B
showed wall-time regressions of up to 10% on some forms (with equal or fewer
instructions), so the runtime keeps the allocating form.

### Intended direction

Give unpacked clockvars with real elements or members a native sample image
and edge input skews a delay, which industry-standard behaviour serves; the
other types stay rejected by decision K1-D1.

### Reproduce

[`neg_string_input.sv`](../tests/fixtures/sim/feature_completion/sim_033/neg_string_input.sv),
[`neg_unpacked_union_input.sv`](../tests/fixtures/sim/feature_completion/sim_033/neg_unpacked_union_input.sv),
[`neg_real_array_input.sv`](../tests/fixtures/sim/feature_completion/sim_033/neg_real_array_input.sv) and
[`neg_edge_skew_delay.sv`](../tests/fixtures/sim/feature_completion/sim_033/neg_edge_skew_delay.sv).

## Synchronous drive forms outside the driver model

**Status:** open (SIM-034 boundary).

### Symptom

One legal form rejects explicitly:

- a runtime select of a clockvar bound to a concatenation (`cb.y[i] <= ...`
  when `y` is declared `output y = {a, b}`): the select would have to pick
  the concatenation part at run time ("a runtime select of clockvar `y`,
  which is bound to a concatenation, is not supported").

### Cause

Synchronous drives reach their target through the clockvar's signal, the
clocking block's own net driver, or the lvalue a clocking expression names,
with each wired net in it retargeted to the clockvar's own driver on that net;
a concatenation's parts are chosen at compile time.

A drive through a virtual interface dispatches on the bound instance with a
`case` over the descriptor's instances, so each drive site carries one drive
per instance of that interface type: code size grows with the instance count
(execution stays a single dispatch). Off-event and `##n` drives keep one heap
record each until they mature; a drive of a clockvar bound to a concatenation
keeps one per part.

### Intended direction

Select the concatenation part at run time with a `case` over the parts, as a
virtual-interface drive selects its instance.

### Reproduce

[`neg_concat_runtime_select.sv`](../tests/fixtures/sim/feature_completion/sim_034/neg_concat_runtime_select.sv).

## Sampled-value arguments without sampled storage

**Status:** open (SIM-035 boundary; element selections of fixed unpacked
arrays are served since sampled-array-cells, see Intended direction).

### Symptom

Sampled-value functions keep a history of packed integral values and of the
exact 64-bit images of real values (`$past`, `$stable` and `$changed` of reals,
and of real cells of fixed arrays, are served by decision AA-D8). Whole fixed
unpacked arrays, unpacked structures and unions, and queues are legal
arguments (SV 16.6.1, L21575 onward: "The following types are not allowed: —
Noninteger types (shortreal, real, and realtime)", then string, event,
chandle, class, associative and dynamic arrays; "Fixed-size arrays, packed or
unpacked, can be used as a whole") but reject explicitly ("sampled histories
hold packed integral and real values" or "must be a static packed or real
expression") instead of being flattened. An element of a queue, dynamic array
or associative array, as a sampled-value argument (`$past(q[i])`) or as a
concurrent assertion operand (`da[i] == 2`), is rejected at the selection
("... selecting a container element ... is not supported" and "unsupported
sampled ... expression ...: llg keeps no sampled value of queue, dynamic array
or associative array elements"). Industry-standard tools reject these element
forms too. Element selections of fixed unpacked arrays (`$past(arr[i])`,
`arr[i] == 2` in an assertion) are served (decision S35-D1).

### Cause

Sampling them needs Preponed snapshots of unpacked or container storage. Fixed
arrays keep one now: descriptor arrays save a cell's old value on its first
change in a time slot, and dense arrays that a sampled expression selects keep
a cell image that each Preponed region updates from the cells written since
the previous one. Containers grow and shrink and
have no such image, and whole unpacked values have no packed sample form.

### Intended direction

Sample whole unpacked values from the same cell images (a whole-array
comparison or `$stable(arr)` reads every cell), and give containers a
Preponed snapshot if element sampling becomes common; until then the
rejections stay explicit.

### Reproduce

Reproduce with
[`neg_unpacked_arg.sv`](../tests/fixtures/sim/feature_completion/sim_035/neg_unpacked_arg.sv),
[`neg_unpacked_struct_arg.sv`](../tests/fixtures/sim/feature_completion/sim_035/neg_unpacked_struct_arg.sv) and
[`neg_queue_arg.sv`](../tests/fixtures/sim/feature_completion/sim_035/neg_queue_arg.sv),
and the container element forms
[`neg_queue_element.sv`](../tests/fixtures/sim/feature_completion/sim_035/neg_queue_element.sv),
[`neg_dynamic_element_operand.sv`](../tests/fixtures/sim/feature_completion/sim_035/neg_dynamic_element_operand.sv) and
[`neg_assoc_element.sv`](../tests/fixtures/sim/feature_completion/sim_035/neg_assoc_element.sv).

## Concurrent property limits after SIM-038

**Status:** open (SIM-038 boundary).

### Symptom

- A procedural concurrent assertion (16.15.6) whose automatic variable or
  `const'` cast is read where the assertion is evaluated outside its
  instances' attempts stops compilation at the assertion: the argument or
  gate of a sampled value function (`$past`, `$rose`, ...), a `disable iff`
  condition, a clock's `iff`, an assertion with global clocking future
  functions, or a static assertion of a procedural checker (17.3). Every
  other read takes the value saved when the instance was queued
  (16.15.6.1).
- An `expect` whose evaluation is disabled by its `disable iff` ends without
  a result and leaves its caller blocked, as 16.18 literally says ("block
  until the given property succeeds or fails").

### Cause

A saved value lives in the frame of one instance's attempt; sampled value
histories, `disable iff` conditions and clock gates are evaluated per
assertion, and a procedural checker's assertions are registered once for all
the procedures that queue them. A disabled `expect` follows the 16.18 text
literally.

### Intended direction

Evaluate the per-assertion forms per attempt where a saved value reaches
them.

### Reproduce

`neg_procedural_automatic_past.sv` in
[sim_099](../tests/fixtures/sim/feature_completion/sim_099/readme.md);
`neg_procedural_disable_iff_automatic.sv` and
`neg_procedural_clock_iff_const.sv` in
`tests/fixtures/sim/feature_completion/ki_audit/`;
`expect_disable_blocks.sv` in
[sim_038](../tests/fixtures/sim/feature_completion/sim_038/readme.md) for the
blocked `expect` caller.

## Multiclock and recursive assertion limits after ADV-013

**Status:** open (ADV-013 and FIN-001 A2 boundary). Recursive properties
with local variable formals or declared locals (such as the LRM's
`fibonacci1` and `check_write_data_beat`) and locals with two semantic
leading clocks are supported since A2 (decision A13-D8); the shapes below
stay located rejections.

### Symptom

- A recursive re-entry whose non-local actuals repeat no enclosing
  expansion (for example three formals rotated) is rejected with "does not
  repeat an enclosing expansion with the same actual arguments".
- A recursive property with local variables whose non-local actuals change
  after the first level (`td(x, local input int k)` called as `td(e, 0)`
  and recursing as `td(d, k + 1)`) is rejected with "re-enters a property
  with local variables after its other actual arguments changed".
- A recursive property whose locals have two semantic leading clocks is
  rejected ("re-enters a property whose local variables are initialized at
  more than one semantic leading clock"), and so is a property whose two
  leading clocks meet under an operator other than `and`, `or` or `not`
  ("would be initialized at more than one semantic leading clock below an
  operator other than ...").

### Cause

A recursive re-entry is a call (`IrPropertyNode::Call`) of an earlier
expansion of the same property. Local variables are passed by value through
a one-tick binding in front of the call, but the non-local actuals are
substituted into the expansion, so they must repeat. The frontend expands a
recursive property at most twice and leaves the next instance without
actuals, so a local formal whose level changes the non-local actuals has no
actual to bind from. A split initialization (16.14.7) has one binding per
leading-clock operand, which a single call cannot repeat; F.4.2 defines the
split for `and`, `or` and `not` only. The empty-match check of a
multiclocked operand is Slang's, which examines the whole operand rather
than the maximal singly clocked subsequence.

### Intended direction

Map the actuals of the unexpanded instance from the expanded copy of the
same call site, and give a split initialization one call per copy. The
frontend check needs a Slang patch.

### Reproduce

`neg_rec_reentry_args.sv`, `neg_rec_locals_args.sv`,
`neg_rec_locals_clocks.sv` and `neg_local_clocks_implies.sv` in
[adv_013](../tests/fixtures/sim/feature_completion/adv_013/readme.md).

## Port connections kept as continuous assignments

**Status:** open; whole variable and `wire` ports share their source's
storage, net ports whose sink has drivers or a net type of its own merge with
their source into one resolved net, whatever net delays or `uwire` sides they
have, and selected, net-array element and concatenated net actuals merge bit
by bit (decisions A21-D1..A21-D5, T1-D4, T1-D5 in
[LRM decisions](lrm_decisions.md)).

### Symptom

A variable sink of a net source runs one copy process: every change of the
source wakes the copy before any reader of the sink, one update later than a
shared storage would show it. A port with a delay on either side whose sink
has no other driver stays a link too, which is the right behaviour (T1-D4).

### Cause

A variable is no net to merge with: sharing its storage with a net source
would let a procedural write or a force of the variable reach the net.

### Intended direction

Keep copies only for conversions, variables and where SDF or specify paths
need each side's own net.

### Reproduce

`tests/sim_port_collapse.rs::kept_links_preserve_each_side`.

## Global clocking future sampled value function limits

**Status:** open (ADV-014 boundary).

### Symptom

An assertion with a global clocking future sampled value function
(`$future_gclk`, `$rising_gclk`, `$falling_gclk`, `$steady_gclk`,
`$changing_gclk`, SV 16.9.4) is rejected at its location ("... with global
clocking future sampled value functions is not supported")
when it is an `expect` statement, when it contains an asynchronous
`accept_on`/`reject_on`, or when the sequence is a sequence end point
(`.triggered`, `.matched`, `@(seq)`). Inside such an assertion a real-valued
operand outside the future function arguments and expression forms other than
operators, selects, casts and concatenations around the future calls are
rejected the same way. A function call as a future function's argument is
rejected at the argument ("$future_gclk argument ... must be a static packed
or real expression"). A future value of a sequence local variable is a
frontend error (16.9.3). The
assertion coverage counters of these assertions count an attempt and its
result when its clock event is evaluated at the next global clock tick (as
the VPI callbacks of 39.4.2.1 are delivered), not at the event itself.

### Cause

These assertions evaluate each clock event at the next global clock tick
from operands captured at the event. An `expect` would block its caller and
an end point would publish its matches one global clock tick late; an
asynchronous abort reads current values between assertion clock ticks
(S38-D13), which the captured operands do not keep. A captured operand is a packed value.

### Intended direction

Capture the abort condition at each of its checks while evaluations are pending,
keep real operands as their 64-bit images, and resume an `expect` caller at
the global clock tick.

### Reproduce

`neg_expect.sv`, `neg_async_abort.sv` and `neg_endpoint.sv` in
[adv_014](../tests/fixtures/sim/feature_completion/adv_014/readme.md); the
operand forms (FIN-001 A3) with `neg_real_operand.sv` (a real operand),
`neg_system_function.sv` (`$countones` of a future value),
`neg_sampled_function.sv` (`$rose` of a future value), `neg_inside.sv`
(`inside` around a future call) and `neg_procedural_automatic.sv` (a
procedural assertion with a saved automatic value);
`tests/fixtures/sim/feature_completion/ki_audit/neg_future_call_argument.sv`
(a call as the argument). A future value of a local variable is rejected by
the frontend first (16.9.3: no local variables in sampled value functions).
Industry-standard behaviour rejects
the `$rose`, `inside` and automatic forms and serves the real operand.

## Checker limits after ADV-015

**Status:** open (ADV-015 boundary).

### Symptom

Free checker variables (SV 17.7, 17.7.2) are randomized and solved, with
these limits:

- A sampled value function (`$past`, `$rose`, ...) of an active free
  variable is rejected ("$rose of active free checker variable `f` is not
  supported").
- An unpacked, real or enum free variable is rejected at its declaration
  (industry-standard behaviour has no free checker variables at all).
- Only predicate assumptions (`C`, `A |-> C`, `A |=> C` with Boolean `A`
  and `C`, no abort) constrain the solve; other assume forms in an assume
  set trigger solving at their clock events but are only checked.

A checker instantiated in procedural code (SV 17.3, 17.3.1) queues its
static assertions on its procedure's procedural assertion queue, but an
actual argument with an automatic variable or a `const'` cast is rejected
at its use in the checker ("automatic variable `v` in a procedural
concurrent assertion takes its value when the assertion is queued").

### Cause

17.7.2 does not require a solution to be found, so llg enforces the forms
whose current-step obligation it can express on the constraint solver.
Sampled histories of variables that are read with current values have no
defined meaning in llg's sampling model. The current value of an automatic
or `const'` actual is substituted whenever a static assertion of the
checker is queued (17.3); llg registers a procedural checker's assertions
once for every procedure that queues them, and saves queued values only for
the procedural concurrent assertions written in a procedure (SIM-099).

### Intended direction

Derive the current-step obligations of sequence and property assumptions
from their pending attempts; lower a procedural checker's actuals in the
procedure that queues its assertions, so they can be saved per instance.

### Reproduce

`neg_free_history.sv`, `neg_free_unpacked.sv`, `neg_free_real.sv`,
`neg_free_enum.sv` and `neg_procedural_automatic.sv` in
[adv_015](../tests/fixtures/sim/feature_completion/adv_015/readme.md); a
`const'` cast actual:
`tests/fixtures/sim/feature_completion/ki_audit/neg_checker_const_cast.sv`.

## DPI-C import limits (SIM-040)

**Status:** open. SIM-040 marshals packed, sized unpacked and open-array
formals of DPI-C imports, aggregates with `real`, `shortreal`, `string` and
`chandle` leaves and dynamic array actuals of open arrays; these forms remain
outside it.

### Symptom

These imports reject with explicit diagnostics:

- a queue actual of an open-array formal (legal by 7.6 and 7.7): the frontend
  reports "value of type 'int$[$]' cannot be assigned to type 'int$[]'";
- a dynamic array of unpacked arrays (`int rows [][3]`) as the actual of an
  open-array formal: "a dynamic array or queue of unpacked arrays as the
  actual of DPI-C open-array formal ... is not supported (SIM-040)";
- a dynamic array actual of an open-array formal whose first dimension is
  sized (`input bit [] a [3]`): "... needs an unsized first dimension of the
  formal (SIM-040)";
- an open-array actual with `real`, `shortreal`, `string` or `chandle`
  elements that is not a whole array variable (a slice, a concatenation, a
  function result): "... must be a whole array variable (SIM-040)";
- an unpacked aggregate formal with packed leaves only that is wider than
  1048575 payload bits;
- a structure formal (not an array of structures) with `real`, `shortreal`,
  `string` or `chandle` members that also holds a member array of 16 or more
  packed cells, the size from which a structure is stored as columns
  ([record columns](#unpacked-records-with-dense-member-arrays-emit-per-cell-code)): "an unpacked
  structure with real, shortreal, string or chandle members and a member
  array of 16 or more packed cells is not supported (SIM-040)";
- an exported subroutine with an unpacked aggregate formal that has `real`,
  `shortreal`, `string` or `chandle` leaves: "unpacked aggregates with real,
  shortreal, string or chandle elements are not supported (SIM-040)".

Two forms the text itself excludes are errors too: a dynamic array actual of
an output open-array formal (7.7, decision S40-D10) and an associative array
actual (the frontend rejects it; an open array has contiguous indices,
H.12.1).

The output and inout actuals of one call are copied back in declaration order
within one kind of formal storage (packed values and strings; real arrays;
containers; native records) and kind by kind in that order, as for any
subroutine call, so an actual named twice through formals of different kinds
(a structure and one of its `real` members) keeps the value of the kind
copied later.

A library that calls a deprecated SV3.1a routine (H.13) fails the model link
with an error naming the routine. A Mach-O library (macOS) is rejected with the same error before
the build, from its symbol table, because ld64 does not check a dylib's
undefined symbols. On Windows a DPI DLL links against the model's import
library (`llg --dpi-import-lib`), which lacks these routines, so the DLL's own
link fails and names the routine (see
[DPI-C export limits](#dpi-c-export-limits-sim-041)).

### Cause

The vendored frontend matches an unsized formal dimension only with a
fixed-size or dynamic array, so a queue never reaches lowering. llg has no
element access for a dynamic array of unpacked arrays. A formal outside the
packed payload is storage of the actual's own type that the caller copies as
a whole, which takes a variable. The packed payload transport ends at the
runtime's width limit. A column-layout structure passes its packed members as
columns and the others in a native value beside them, and the thunk converts
one storage form per formal. The export thunks convert only packed payloads. H.13
lets an implementation leave the SV3.1a routines out.

### Intended direction

Admit a queue for an unsized dimension in the frontend's open-array matching
(a vendored-frontend patch); the lowering and thunk already treat a queue
like a dynamic array. Build the copy of a non-variable actual at the call
site. Convert a column-layout structure from its columns and its native value
together. Give the export thunks the import's conversions for native leaves.

### Reproduce

`tests/fixtures/sim/feature_completion/sim_040/neg_*.sv` and
`tests/fixtures/sim/dpi/unsupported_native_member.sv` (export).

## DPI-C export limits (SIM-041)

**Status:** open. SIM-041 provides exports, `svScope` and context call
chains, and ADV-018 exported tasks that consume time; these cases remain
outside them.

### Symptom

- A context imported task of a design with an exported task that can
  consume time runs its C code on a foreign call context (an OS thread with
  its own stack, [design](../src/sim/rt/foreign_contexts/readme.md)), whether
  or not that call reaches such a task: each call costs two thread
  hand-offs, and each call of a waiting exported task two more. Measured on
  the x86-64 development host: 0.13 to 0.25 us per hand-off while the other
  thread still polls on a core of its own, about 18 us once it sleeps (the
  other side ran longer than the poll bound), and about 19 us when both
  threads share one CPU, where the poll only delays the other thread. The C code must not rely on running on
  the main thread (thread-local storage, thread affinity of other
  libraries). Its stack is `LLG_DPI_CONTEXT_STACK_BYTES` (8 MiB by default;
  set it with `LLG_CFLAGS=-DLLG_DPI_CONTEXT_STACK_BYTES=<bytes>`).
- A chain still suspended when the simulation ends is never resumed
  (A18-D2): its thread stays blocked until the process exits.
- On Windows a DPI DLL (or VPI plugin) reaches the exports and the `svdpi.h`
  and VPI routines in `sim.exe` through the model's import library, written by
  `llg --dpi-import-lib <file>` for the same design and options before the
  DLL is linked. The DLL must export its import functions (`DPI_DLLESPEC` or a
  `.def`), `--dpi-lib` takes the DLL's import library, and the DLL must stay
  beside it (llg puts that directory on `PATH`; a model started by hand needs
  it on `PATH` too). The executable must keep the name `sim.exe`, which the
  import library names. A call of an export the design does not declare fails
  the DLL's link instead of the model's. Only MSVC toolchains build the import
  library; a MinGW DLL is not supported (static archives work everywhere).
  Build the DLL with `/MD`, the shared UCRT the model is built with: `/LD`
  alone implies the static CRT (`/MT`), whose own `stdout` buffer keeps the
  DLL's `printf` output until the DLL unloads, after every line the model
  printed. llg cannot flush another C runtime's buffers.
  The generated `sim.def` and CMake text are unit-tested on every host; the
  CI `Windows x86_64` `test` job builds every DPI-C and VPI companion of the
  suite as a DLL against the import library (`sim_040`, `sim_041`, `adv_018`,
  `sim_042`, `adv_020`, the S40/S41/A18 decision cases).
- On macOS ld64 does not check the undefined symbols of a dylib built with
  `-undefined dynamic_lookup`, so llg reads each Mach-O `--dpi-lib` symbol
  table before the build and names an import that neither the model (the
  `svdpi.h` and VPI routines, the design's DPI-C exports and imports) nor
  another `--dpi-lib` defines. When a `--dpi-lib` is not a Mach-O file (a
  static archive), whose definitions are not read, only unprovided
  `svdpi.h` routines are named; another missing symbol is left to dyld, which
  ends the model without a message when it is called. A Mach-O library that
  expects such a symbol from a dylib it was not linked against is reported
  too.
- An export called outside any DPI-C call chain (for example from a C
  `atexit` handler or another thread) is a fatal error; exports are not
  thread-safe.
- Recursion through C (SV -> C -> SV ...) uses the native stack of the
  process for each level; the 256-call recursion limit bounds it, but deep
  levels with large C frames can exceed small thread stacks.
- An export function that disables a block which called it is undefined in
  SV 9.6.2; llg follows the 35.9 protocol for it (S41-D5).

### Cause

The C frames of a suspended chain must stay on a native stack of their own;
threads are the portable way to keep one (the design weighs the
alternatives). Windows DLLs resolve imports only from libraries named at
link time, so the model's import library must exist before the DLL and must
name the executable.

### Intended direction

A faster context switch (fibers) behind the same interface if the hand-off
cost matters; a `dlltool` import library if MinGW DLLs are needed.

### Reproduce

`tests/fixtures/sim/feature_completion/adv_018/crossing.sv`;
`tests/fixtures/sim/lrm_decisions/S41-D1_context_scope_chain.sv`. Each
point above has a test:

- Foreign call contexts only where needed:
  `adv_018::only_designs_with_waiting_exports_get_context_shims`; their
  stack size and its flag: `adv_018::context_stacks_have_the_configured_size`.
- A chain suspended at the end: `sim_lrm_decisions::a18_d2_finish_leaves_chain_suspended`.
- The Windows DLL route: the DLL lanes above on the CI `Windows x86_64`
  `test` job, and `sim_040::model_import_library_is_explicit_about_the_toolchain`
  (an import library under MSVC, the explicit error elsewhere).
- macOS: `sim_040::unprovided_routine_imported_by_a_mach_o_library_is_named_before_the_build`
  and `sim_041::undeclared_exports_imported_by_a_mach_o_library_are_named_before_the_build`.
- An export outside a call chain: `sim_041::export_called_outside_a_call_chain_is_fatal`.
- Recursion through C: `sim_041::recursion_through_c_stops_at_the_recursion_limit`.
- S41-D5: `sim_lrm_decisions::s41_d5_disabled_outputs`.

## Strings declared in a branch of a function

**Status:** a `fork`-`join_none` branch of a function runs after the function
returned, so it has no activation of that function to name. Class handles and
records declared in the branch are branch locals, formals are captured, and
instance properties of `this` (containers and records included) are reached
through the captured receiver. One form remains.

### Symptom

A string declared in the branch is rejected (`string/class signals are not
supported`).

### Cause

Block-level string storage is a global per declaration, chosen for procedural
blocks with one activation; the detached branch has no lexical string local.

### Intended direction

Declare the string as a branch-local native value, as records are.

### Reproduce

```systemverilog
class c;
  function void f(int n);
    fork begin string s = $sformatf("v%0d", n); #1 $display(s); end join_none
  endfunction
endclass
```

Fixture: `tests/fixtures/sim/feature_completion/ki_audit/neg_string_in_function_fork.sv`.

## Constrained randomization limits (SIM-029, SIM-030)

**Status:** open. SIM-029 and SIM-030 solve packed integral and enum
variables, `randc`, `dist`, `solve...before`, fixed, dynamic and bounded
random arrays of packed elements (fixed ones of any dimension count), array
methods and `foreach`, and random object members;
these forms remain outside them.

### Symptom

- Rejected with a located `randomization:` error: nested dynamic arrays and
  queues, associative arrays of class handles or with a string, class,
  wildcard or wider than 64-bit index type, the size of a random associative
  array, a random key and one element of an associative array as a
  `solve...before` side (S30-D20), `randc` arrays and `randc` variables wider than
  32 bits (18.4.2 lets an implementation limit them; S30-D18), a random untagged union
  with enum members, a `solve...before` that solves a variable before an
  array size when a constraint ties that variable to elements of an array of
  random size (`x == a.sum()`; S30-D19 fixes the variable before any element
  exists, so the order would be circular), that solves array elements before
  an array size or a variable before the array size of another object, or that
  names an element at a non-constant index, and a row of a multidimensional array in `inside`. A whole array in
  a constraint is also rejected, as 18.3 requires (S30-D15).
- `rand_mode` of one element of a dynamic array or queue of class handles
  rejects, as a task and as a function: only fixed arrays keep a mode per
  element (S30-D16, S30-D17). The function form on a handle that is not a
  random variable (`e.rand_mode()` of a module variable or of a property that
  is not `rand`) stops in the frontend, which reads it as the object task.
  `rand_mode` of one element of an array of packed values
  (`c.v[1].rand_mode(0)`) rejects too.
- A constraint function whose receiver is an element of a random handle
  array (`foreach (q[i]) q[i].v >= q[i].f(x)`) rejects with a located error.
  Every object a randomize call reaches evaluates its own functions, also
  objects of the randomized class and the randomized object reached again
  through a ring, and a scalar handle receiver (`next.f(x)`) works.
- A `randomize()` call used as a value works in assignments, `if` conditions,
  `return`, declarations, system-task arguments, immediate assertions, loop
  headers and short-circuit or conditional operands of those. Elsewhere (a
  `wait` condition, an event control) it rejects.
- When the exact model and rejection sampling both exceed their budgets, the
  CDCL search returns a legal solution that is not uniformly distributed
  (S29-D10), against 18.5.9 L30013-30014; the fallbacks also ignore dist
  weights and solve-before orders (S30-D9). The run prints one warning per
  call site naming the site and the budget (`LLG_RAND_BDD_NODES` or
  `LLG_RAND_BDD_BITS`) to raise.
- A loop that keeps needing the search pays the search on every call (about
  18 ms per call for a 16-bit factoring constraint on x86-64, almost all of
  it search: the encoding is reused while the state values repeat and is
  about 1 to 20 percent of a call otherwise).
- A component whose exact model exceeds `LLG_RAND_BDD_NODES` spends the whole
  node budget before falling back (about 0.4 to 0.9 s at the default budget
  for that constraint, depending on memory load). The overflow is remembered
  per state, so repeated calls with the same state values skip it. When the
  state changes, the second overflow of the component proves whether its
  state-free part alone exceeds the budget (a product of random variables
  compared with state does); if so, later calls skip the build without
  changing any drawn value. An overflow that needs the state-dependent part
  (a product with a state operand) is still paid once per new state value;
  lowering `LLG_RAND_BDD_NODES` trades exact uniform sampling of larger
  components for speed.
- A problem with arrays or objects is expanded per call shape (instances,
  array sizes); a new shape costs one expansion, bounded by
  `LLG_RAND_ARRAY_ELEMENTS` and `LLG_RAND_EXPAND_NODES` (a resource error
  when exceeded). Each call site keeps the sixteen most recently used shapes.

### Cause

The exact engine counts models over the whole component, which a product of
wide variables can exceed; the CDCL fallback finds one solution without
counting. The other rejected forms need storage or expansion paths the
lowering does not have.

### Intended direction

Nested dynamic arrays and queues (each row an object of a row unit, row sizes in a further size phase), `randc` arrays (a cycle state per element) and element modes of dynamic arrays and queues if a workload needs them; near-uniform sampling for the search fallback (hashing-based).

### Reproduce

`tests/fixtures/sim/feature_completion/sim_029/neg_*.sv`,
`tests/fixtures/sim/feature_completion/sim_030/neg_*.sv` and
`search_fallback.sv`; `neg_rand_*.sv` and `neg_randomize_event_control.sv` in
`tests/fixtures/sim/feature_completion/ki_audit/` (associative arrays of
handles, with a class or wider index, nested queues, a non-constant
`solve...before` index, an `inside` row, an element `rand_mode`, a call in an
event control); `tests/fixtures/sim/lrm_decisions/S29-D10_search_not_uniform.sv`.

## Legacy constructs unsupported by design

**Status:** decided 2026-10-08; not planned. The rows are marked 🚫 in
[the feature checklist](sim_features.md).

### Symptom

Procedural `assign`/`deassign` in every form (local or hierarchical target;
decided 2026-10-09), MOS and resistive switch primitives, `trireg` nets,
the `` `default_decay_time``, `` `default_trireg_strength`` and `` `delay_mode_*``
directives, `$dumpports*`, `$q_*` outputs that are not whole integer variables,
the PLA tasks, `$countdrivers/$getpattern/$scale/$scope/$showscopes/$showvars`
and the PLI 1.0 `tf_*`/`acc_*` interface stop the run with
`error: <file>:<line>:<col>: unsupported: <construct> (<family>) is not
supported by llg` before C generation.

### Cause

`sim::legacy_unsupported::scan` finds the constructs the owned database exposes
(primitives, nets, procedural `assign`/`deassign` statements, system calls and
directives) and lowering rejects the remaining forms with the same message
shape. Hierarchical continuous assignments and force/release are supported and
not part of this list. The PLI 1.0 check cannot be
exact: `--dpi-lib` libraries are scanned for the symbol strings `veriusertfs`
and the `tf_*`/`acc_*` routine names, so a library that does not spell them
(an obfuscated or runtime-computed name), an unlinked source that only includes
`veriuser.h` or `acc_user.h`, or a defining-only library that exports a user
function with one of those names is judged by the scan alone. Slang's own
errors for `assign` to a net, select or streaming target are kept as they are.
The scan matches system calls by name only; whether a VPI task registered under
one of the rejected built-in names is distinguished was not checked.

### Intended direction

None: the decision is to reject.

### Reproduce

The files in
[adv_032](../tests/fixtures/sim/feature_completion/adv_032/readme.md).

## VPI object model limits (SIM-042)

**Status:** open. The object model catalog
[`src/sim/rt/vpi_catalog.txt`](../src/sim/rt/vpi_catalog.txt) is the single
source of truth for which object types, properties, one-to-one relations,
iterations, callback reasons and `vpi_control` operations the runtime
serves: every row it marks `unsupported` is a gap of
this section, with its reason, and a query no row lists is an error
(decision A19-D26). This section lists the limits the catalog does not
express: objects outside the generated catalog, values, callbacks, routines
and registrations.

### Symptom

These VPI requests return NULL, `vpiUndefined` or 0 with a `vpi_chk_error`
error (`LLG_VPI_UNSUPPORTED`, `LLG_VPI_INDEX` or `LLG_VPI_NOT_FOUND`) instead
of a handle or value; `vpi_handle_by_name` of such an object returns NULL
with no error status, as for a name that denotes nothing (S42-D11):

- objects outside the generated catalog: the automatic variables of
  process blocks (`initial`/`always` blocks have no frame) and of a fork's
  blocks inside a task or function; a block of a task or function has no
  frame of its own (decision A19-D76: `vpiFrame` stays the subprogram's;
  its locals are served by name and by `vpiVariables` of the block while it
  runs, decision A19-D88). A checker has no 2009 VPI object: a checker
  instance is a module scope with its variables (decision A19-D88);
  `cbForce`/`cbRelease` callbacks on the elements of a sparse fixed array
  (one kept in a keyed container; its values, puts and value-change
  callbacks are served, decision A19-D88);
- dynamic objects (VPI-O4) other than string, chandle and class variables,
  dynamic, queue and associative arrays (nested ones flattened to their
  innermost elements), fixed-size arrays of strings, chandles, class
  handles and unpacked structures of any number of dimensions, the elements
  of arrays of structures with their members, frame automatics of those
  kinds, static class properties, fixed-size class instance properties and
  the members of structure elements and properties (decisions A19-D73,
  A19-D76, A19-D82, A19-D87, A19-D89): fixed-size arrays of
  other element types not stored as packed values, events other than event
  properties of classes (decisions A19-D45, A19-D87), and virtual interface
  variables (virtual interface properties of classes are served, decision
  A19-D72);
  mailbox and semaphore objects (`vpiClassType` and `vpiMessages`; their
  typespecs are NULL; `vpiWaitingProcesses` is served, decision A19-D71);
  `foreach` constraints (no 2009 object, decision A19-D37); `vpiTypedef`
  of class definitions and objects,
  `vpiArgument` of `vpiExtends` and `vpiParamAssign` of class typespecs; a
  parameterized class lists one class defn per specialization, each under
  the class's own name (decision A19-D36);
  the members of a structure kept as one packed value in a class property
  or dynamic element, and bit and part selects of a dynamic element, member
  or property (NULL with an error, decision A19-D89; industry-standard
  behaviour serves them), associative arrays
  with a wildcard index or keys of more than 64 bits; `vpiFullName` of an
  element of a container property of a class object (37.17 detail 25
  gives a non-static class member no full name; industry-standard
  behaviour names it by the path that reached it, `tb.p.da[1]`, and
  answers NULL when the object was reached by iteration; pending a
  decision); `cbValueChange` on class properties, on elements of
  fixed-size chandle arrays (industry-standard behaviour serves these),
  strings and arrays (the last two excluded by 38.36.1); `vpi_put_value` with a
  delay, force or release on a string, property or element (decisions
  A19-D21, A19-D23);
- structure objects (ADV-019 family 2; their relations are in the
  catalog): elements of multidimensional primitive arrays, and `vpiIndex`
  of an element of a multidimensional instance array; `vpiReturn` of an automatic function and the argument
  variables of an automatic task or function, and the string, chandle,
  class and event arguments of a static one (decisions A19-D53, A19-D57); a fork branch started inside a task has no frame of that task
  (its frame stack starts empty);
  `cbValueChange` on a continuous assignment or primitive (38.36.1 lists
  variables, expressions, terminals and events; industry-standard tools
  refuse both too, decision A19-D91), and on an operation or a primitive
  terminal (industry-standard behaviour serves both; watch the variables
  and nets they read or drive); a delayed
  `vpi_put_value` to a named event (only `vpiNoDelay` triggers, decision
  A19-D8). A `cbValueChange` on a named event watches the synchronization
  object the event names at registration;
- connectivity (decisions A19-D33 to A19-D35): a force statement counts as
  active while a live binding covers exactly its target bits, so two force
  statements with the same target are reported together while either is in
  effect; the connections of string, chandle, class and container
  variables, of interface and modport port references and of a port whose
  module-side expression is a concatenation (a multi-port has no low
  connection) are not indexed; a port declared with a module-side
  concatenation (`.q({y1, y0})`) on an element of an instance array has no
  `vpiHighConn` (decision A19-D58);
- structure and union members that are not exposed (decision A19-D89):
  an unpacked array member of strings, handles or structures that llg keeps
  one leaf per cell (`vpiMember` lists the other members and reports an
  error; the member and its elements are NULL with an error), bit and part
  selects of a member of an element of an array of structures
  (`tb.ea[1].k[1]`; industry-standard behaviour serves these), and unpacked
  structures inside an unpacked union (industry-standard behaviour rejects
  unpacked unions); `vpiElement` of a `vpiPackedArrayVar` (its elements
  and bits are reached by index or name, decision A19-D67);
- typespecs (the served properties and relations are in the catalog):
  a reference object (an interface or modport port connection, or a
  virtual interface property of a class) has no typespec, as IEEE
  1800-2009 defines no interface typespec (decision A19-D90;
  industry-standard behaviour serves one of a vendor-specific type);
  class typespecs are reached from class variables and objects (VPI-O4);
- subarray handles: `vpi_handle_by_index`, `vpi_handle_by_multi_index` and
  `vpi_handle_by_name` with fewer indices than the array has unpacked
  dimensions return NULL with an error (decision A20-D23: the 2009 object
  model has no subarray object), and so do subarrays of a packed array
  with three or more dimensions (`pb[1]` of `bit [1:0][2:0][3:0] pb`,
  decision A19-D67); a part-select of a packed array element
  (`pa[2][3:0]`) is not served by name;
- expression and statement objects (ADV-019 families 3 and 4, decisions
  A19-D11 to A19-D13) exist only in models built with `--vpi`, `--dpi-lib`
  or a user system task call site. `vpi_get_value` of a function call, a
  system function call or an expression with side effects (`i++`, an
  assignment), and of every expression containing one, fails, although
  38.15 asks for a full evaluation including the side effects (a full
  evaluation needs a run-time expression interpreter with function calls;
  industry-standard tools do not evaluate them either: they fail on calls
  and return the last value of the other forms without their effects). Assignment
  patterns, streaming concatenations, `<->`, tagged and `let` expressions,
  sequence and property expressions, class method calls and members reached
  through a hierarchical path are opaque objects: their type, size and line
  only (relations, values and `vpiDecompile` fail); so are pattern `case`,
  `randcase`, cycle delays, `expect` and assertion statements. A reference to
  a variable outside the catalog (an automatic variable or argument) is an
  object with its name and type but no value. A labeled statement appears as
  a named block holding the statement (37.56 detail 1 gives the statement
  itself the label as `vpiName`). Among expressions only casts have a
  `vpiTypespec`, only selects a `vpiParent`; statements have no `vpiParent`.
  `vpiUse` lists the enclosing statement of a procedural use (A19-D43). An argument that is not a whole object keeps
  the size and value of the formal it is passed to (a call snapshot) while
  its relations and `vpiDecompile` are those of the expression;
- source provenance (37.3.3): `vpiLineNo` and `vpiFile` fail with an error
  on typespecs without a typedef, enum constants, typespec members, and the
  ranges and range bounds of a typespec that no declaration owns (an enum's
  base type, a structure member's type), as industry-standard behaviour
  gives typespecs, enum constants and typespec members no line (decision
  A19-D86; `adv_019::catalog_walk_matches_the_vpi_catalog` lists them);
- every property, relation and iteration the catalog marks `unsupported`
  (after the ADV-019 closure and the sequential UDP put, A19-D74, 2026-11:
  none). An event of a class definition's event array property
  is reached only by index (it names no event: no value, waiters or put,
  decision A19-D85), and an event of an event array property takes no
  `cbValueChange`, as an event property does not (decision A19-D80). Rows the catalog marks `error` are refused on purpose,
  as industry-standard behaviour refuses them (decision A19-D65); among
  them the bodies, variables and formal defaults of sequence and property
  declarations, `expect` relations, the list operation of an instance
  array's connections and a structure member's default expression; pattern
  objects are `n/a` for comparison (industry-standard tools reject
  `case ... matches`) and keep llg's opaque objects;
- callback reasons refused with an error naming the reason: `cbStmt` (a
  statement has no execution hook; adding one would cost every model
  statement a check, or a second statement-hooked code path per process),
  `cbAssign`/`cbDeassign` (procedural `assign`/`deassign` is rejected,
  decision A20-D9), `cbUnresolvedSystf` (every system call is resolved when
  the model is generated) and the save, restart, reset and interactive
  reasons (no such features). `cbError` reports run-time error reports, not
  compile errors (decision A20-D20); `cbSignal` catches the signal with the
  C `signal()` handler, replacing the model's own;
- the `vpiForce`/`vpiRelease` handle of a `cbForce`/`cbRelease` callback has
  only `vpiType` and `vpiLhs`, and `vpiLhs` is NULL with an error for a
  concatenation (no expression objects); `cbDisable` on a system task call
  watches the named blocks enclosing it (its `vpiScope` chain), so a call in
  a task body does not see `disable` of the task;
- `cbValueChange` on a whole unpacked array of more than
  2^20 elements (register it on the elements); `vpiStrengthVal` reads and records
  report real strengths for resolved nets and their selects only: elements
  of net arrays and nets the model keeps without a resolution function
  report strong (or `vpiHiZ`) values and no strength-only changes;
- `vpi_control(vpiReset)` and `vpiSetInteractiveScope` return 0 with an
  error (no `$reset`, no interactive mode; decision A20-D11);
- `vpi_put_value_array` to a net array (its elements are read-only to VPI,
  as for `vpi_put_value`; decision A20-D8; serving it needs a per-element
  override until a driver changes, like a force without its release);
- delays (VPI-T1, VPI-F8): `vpi_get_delays`/`vpi_put_delays` serve module
  paths, timing checks, primitives, input and inout ports and port bits
  (module input port delays) and intermodule paths; continuous
  assignments, nets and delay controls are read-only; a module or an output
  port is a `vpiError` (decisions A20-D12..D19). `vpi_put_delays` needs a
  model generated with `--vpi` or `--dpi-lib` (it reserves the writable
  delay tables then; otherwise the write is refused) and leaves
  transitions already scheduled at their times. A primitive whose delay is
  an expression of variables keeps a written delay until one of the
  expressions changes value; one that changes and changes back before the
  primitive's next transition keeps the written delay. Port delays need a
  port connected to a net of its own that no module path, switch or alias
  drives (otherwise the write is refused). By default a port between two
  nets is one simulated net with its connection (decision A21-D6), and its
  delays, those of its bits and of an intermodule path to it are refused;
  `--vpi-port-delays` keeps every input and output port a net of its own
  for the whole model (some simulators enable this per module), and an
  inout port between two nets is always one simulated net. The delays of delayed timing
  check signals stay fixed, as for SDF. There are no delay device or delay
  term objects (37.41);
- `vpiTchkTerm` and `vpiPathTerm` handles have no own source location;
- `vpi_handle_multi` with a type other than `vpiInterModPath` (no
  `vpiPortInst` objects) and the routines `vpi_get_data`/`vpi_put_data` (no
  save/restart).
  They are declared and defined so that applications link, and return
  NULL, 0 or nothing with an `LLG_VPI_UNSUPPORTED` error without reading or
  writing their arguments;
- `vpi_put_value` with force or release on strings, class properties and
  container elements, and delayed puts to properties and elements
  (decision A19-D23), and delayed puts to named events (A19-D8).

System task and function registrations are also limited (SIM-099):

- a `vpi_register_systf` under the name of a built-in llg implements does
  not override it, although SV 36.3.2 says the application "shall override
  the built-in system task or system function". When the design calls that
  built-in, the run stops at startup naming it and its call (decision
  S99-D6); a registration of a built-in the design does not call is
  accepted. Built-ins llg does not implement (the Annex D services such as
  `$key` and `$reset_count`) are dispatched to such a registration;
  `$sreadmemb/$sreadmemh` (ADV-026) and `$sdf_annotate` (ADV-012) are
  built-ins;
- a `string` formal (`$input`, `$log`, or a
  `--define-system-task` `string` formal) accepts only a string literal, which
  the application reads as a `vpiConstant` string; a string variable or
  expression fails to compile with `<file>:<line>:<col>: argument of VPI
  system task `$input`: a string variable or expression is not supported`,
  because a string has no fixed-width argument image. Scope actuals reach the
  application as module (or named generate scope) handles only for `$list`.

A model built without `--vpi`, `--dpi-lib` or a user system task call site
has no object catalog: an application loaded with `LLG_VPI_PLUGIN` gets
callbacks but finds no objects, and the model warns on stderr.

Elements of a net array are read-only through VPI (they are published from
their nets). A `vpiReleaseFlag` put on a variable that a continuous assignment
drives leaves the forced value until that assignment's inputs change: the
re-evaluation an HDL `release` triggers is generated for HDL release sites
only.

### Cause

The catalog is emitted from the owned database's static storage; everything
else would need runtime-owned handle targets with GC pins (containers, class
objects) or descriptor tables the emitter does not produce yet.

### Intended direction

Extend the catalog per object family with the same generation-checked slots;
pin collectable targets in the handle slot, and turn the catalog row from
`unsupported` to `implemented` with its test.

### Reproduce

[sim_042](../tests/fixtures/sim/feature_completion/sim_042/readme.md) and a
plugin that asks for one of the objects above, or with the ADV-019 catalog
walk (`tests/fixtures/sim/feature_completion/adv_019/catalog.c`).

## Container index-check stops name no array or source line

**Status:** open by cost (S07-D15 stops are served).

### Symptom

With index checks on (S07-D15), a queue `insert` or `delete(index)` with an
invalid index, an associative-array access through an X or Z key and a
`new[]` with a negative or X/Z size (S07-D17) stop the run with a message that
names the operation, the index, key or size, the valid range and the current
process, but not the array or the source line. The run
ends at once with status 1: final procedures do not run and an open waveform
file is not completed.

### Cause

These checks sit on the invalid path inside the container runtime, which
receives neither a name nor a location, so valid accesses pay nothing; the
generated code after a container call has no exit check, so the runtime
cannot hand the stop back to the process.

### Intended direction

Pass a site index to the container calls only where a cheap static table can
name the array and line, if the cost measures as zero.

### Reproduce

`q.insert(5, 9);` on a two-element queue.

## VPI work during static initialization

**Status:** open.

### Symptom

A user VPI system function called from a static declaration initializer
runs (decision S43-D12), but work its calltf schedules there, such as a
`vpi_put_value` with a delay, is not reported as the S13-D5 error that SV
13.4.4 requires for deferred work outside a procedure's thread: the put
matures at its time. A call from it into a DPI-C export is a call outside a
DPI-C import (SV 35.5.3).

### Cause

The static-initializer walk of S13-D5 follows SystemVerilog calls,
constructors, virtual overrides and, from a context DPI-C import, every
export. Foreign VPI code is opaque to it, and the run-time check for a
process spawned with no parent covers only a `fork`.

### Intended direction

Check the VPI routines that schedule work against the static-initialization
phase at run time, as a parentless fork is.

### Reproduce

`tests/fixtures/sim/feature_completion/ki_audit/vpi_static_initializer.sv`
with `vpi_static_initializer.c`: `int y = $sched();` at module scope, where
the calltf of `$sched` calls `vpi_put_value` on a variable with
`vpiInertialDelay`; the run prints `x=5 y=7` without the S13-D5 error
(`ki_audit::vpi_function_called_from_a_static_initializer_runs_its_delayed_put`).

## Functional coverage limits (SIM-044, SIM-045)

**Status:** open. SIM-044 and SIM-045 provide covergroup instances, the
1800-2009 bins, transition and cross forms, and the declared options; these
legal forms remain outside them.

### Symptom

These covergroups reject with located diagnostics:

- a coverpoint wider than 128 bits, or a bins value read from a covergroup
  argument wider than 64 bits;
- in a coverpoint of 65 to 128 bits: a bins value read from a covergroup
  argument, an item of more than 2^32 values in a `[]` or `[N]` bins array
  (or a `[]` transition bin), `ignore_bins` or `illegal_bins` beside
  automatic bins, `intersect` on it when it has automatic bins, and an enum
  type;
- a `wildcard` bins value whose don't-care bits expand to more than 4096
  ranges;
- transition bins whose expansion exceeds a compile-time budget of one
  coverpoint: 65536 automaton states, 4096 sequences of one `[]` or removal
  bin, 2^20 start entries, or 2^20 value class sequences to count its bins;
- transition values read from a covergroup argument in a `[]` transition
  bin, in a coverpoint with `ignore_bins` or `illegal_bins` transitions, or
  in a coverpoint whose `option.detect_overlap` may be set;
- a cross of a coverpoint that has transition bins, and a `binsof` that names
  a transition bin;
- a cross select expression that expands to more than 1024 terms;
- a block event on an imported or extern subroutine, which has no body;
- a `ref` argument whose actual is not a whole static variable or net.

At run time:

- a cross whose select expressions split its coverpoints into more than
  2^20 value-class combinations reports a located error when the instance is
  constructed and counts no bins;
- with `type_option.merge_instances`, a cross whose instances have different
  cross bins (bins depending on covergroup arguments) reports an error, and
  its merged coverage is 0;
- `option.detect_overlap` stops checking transition bins after a fixed
  budget of explored sequence steps; compilation then warns, located at the
  coverpoint, which pair of bins it stopped at, and overlaps from that pair
  on are not reported.

These options are stored, can be read back and written where 19.7 allows,
but change no number, because their only effect in Table 19-1 and Table
19-3 is on the coverage report, which llg does not write (a report through
`$get_coverage` text or a report file is out of scope until a user asks for
one):

- `option.goal` and `type_option.goal`: the target a report compares
  coverage with;
- `option.comment` and `type_option.comment`: saved in the coverage
  database (with `option.name`), shown in no report;
- `option.cross_num_print_missing`: the missing cross products a report
  lists; the database keeps none;
- `option.per_instance`: llg always keeps instance coverage, which Table
  19-1 allows when the option is 0 ("implementations are not required to
  save instance-specific information"), so `get_inst_coverage()` reports it
  either way. Some simulators return 0 from `get_inst_coverage()` when the
  option is 0; portable fixtures set it.

### Cause

`ref` arguments are bound statically per distinct actual tuple. Bins values
are 64-bit runtime keys; a wider coverpoint maps its values to them through
value classes built from its constant bins values, so per-value bins need a
key per value and argument values cannot be placed. Transition bins are compiled to fixed automata;
when a value reads a covergroup argument, each instance builds its own value
classes when it is constructed, but the searches that count `[]` sequences,
remove ignored and illegal sequences and find overlaps run only at compile
time, on constant values. Crosses address coverpoint bins by value
rank, so transition bins have no coordinate. Merged cross bins are matched
by layout (coverpoint-bin coordinates and select signatures), not by the
19.11.3 `<binname1,...,binnameN>` names; a name-keyed cross merge would need
the bin names of every dimension per instance, which the store does not keep.
Instances, and the per-instance event waiters that hold them, live until the
end of simulation (S44-D3, S44-D5), so memory grows with every covergroup
object created.

### Intended direction

None of these limits has an owner yet; the query, control and database
services of SIM-046 have their own limits below.

### Reproduce

`tests/fixtures/sim/feature_completion/sim_044/neg_*.sv`,
`tests/fixtures/sim/feature_completion/sim_045/neg_*.sv`,
`merge_cross.sv`, `cross_classes.sv` and `overlap_budget.sv`.

## Coverage API and database limits (SIM-046)

**Status:** open. SIM-046 provides the 19.8-19.9 query and database
services for covergroups and the 40.3.2 coverage API, with its VPI
counterpart, for assertion coverage; ADV-017 adds FSM state coverage from
the 40.4 pragmas, and statement and toggle coverage follow S46-D13,
S46-D14 and A19-D46.

### Symptom

- Statement and toggle coverage are collected only in a model whose
  `$coverage_*` calls name them or that is built with `--code-coverage`
  (`simulator.code_coverage`) for them (both from time 0), or that is built
  with `--vpi` or `--dpi-lib` (stopped until a START). A model that loads a
  VPI application at run time (`LLG_VPI_PLUGIN`) without any of these has
  neither kind: queries return 0 and CHECK reports `SV_COV_NOCOV`. The
  option is a generation option: a built model cannot change its kinds at
  run time.
- Toggle coverage counts packed integral nets and variables of instances,
  their generate scopes and ports; unpacked arrays, reals, strings,
  events, handles, class properties, and block, subroutine and package
  variables have no toggle items (S46-D14). Coverage is reported per bit;
  no 1800-2009 query reports the 0->1 and 1->0 counts apart.
- A string scope argument names a module definition only; nested
  definition notation (`"top.sub"` naming a definition inside another) is
  `SV_COV_ERROR` (S46-D6).
- The database is llg's own versioned format, not UCIS or another
  simulator's: a database is read only by a model with the same
  covergroups (and, for `$coverage_merge`, the same assertion items). A
  different model is rejected as a whole; records are not matched by name
  across designs.
- Type options (`type_option.weight`, `goal`, `comment`,
  `merge_instances`) are saved (format version 2), but a load applies none
  of them: a loaded run uses the current model's type options, as
  industry-standard behaviour does (S46-D15). No llg tool reads the saved
  values yet; a database of format version 1 still loads.
- VPI assertion API (ADV-017): assertion handles answer the static 39.3.2
  properties, `vpiFile`/`vpiLineNo`/`vpiColumn` and the 40.5.3 counters,
  and run the 39.4 callbacks and 39.5 controls, with these limits:
  - the relations of an assertion (`vpiClockingEvent`,
    `vpiDisableCondition`, `vpiProperty`, `vpiExpr`, `vpiStmt`,
    `vpiElseStmt`) and `failExpr` exist only in models generated with
    `--vpi` (or `--dpi-lib`); sequence and property operators are
    operations with operands but no value (A19-D59). `strong(...)` and
    `weak(...)` around a Boolean expression or a sequence instance leave no
    trace, match items after a Boolean expression that is a variable are
    not kept, and `.triggered`/`.matched` are method calls that do not
    decompile;
  - a sequence or property declaration keeps no body, variables or formal
    defaults (`vpiPropertySpec`/`vpiExpr`/`vpiVariables` of a declaration,
    `vpiExpr` of a formal fail with `LLG_VPI_UNSUPPORTED`: each instance
    holds its expanded body and its actual arguments), and declarations are
    not found by `vpi_handle_by_name`;
  - callbacks on a sequence or property instance run only for an instance
    that is the whole property of an assertion; a nested instance refuses
    them, and the sequence instance of a `cover sequence` gets no failure
    (A17-D5);
  - assertion stepping: `vpiAssertionEnableStep` and
    `vpiAssertionDisableStep` fail with `LLG_VPI_UNSUPPORTED`, so a step
    callback (`cbAssertionStepSuccess`/`StepFailure`) registers but never
    runs (A17-D2). A step
    (39.4.2) reports the expressions matched and the source and destination
    states of the F.5.1 rewriting; llg's simple, sequence, property and
    future engines number no such states and record no matched leaves, so
    a step callback could only be a stub. Providing them needs a state
    numbering shared by all engines and a per-thread hook behind the item's
    `watches` gate (no cost without a registration);
  An assertion label that is also
  listed by the object catalog as a named block is found as the assertion
  by `vpi_handle_by_name`.
- `vpi_get(vpiAssertCoverage, instance)` and `vpi_control` with an
  instance handle select that instance's own assertions only, as
  `SV_COV_MODULE` (S46-D10).
- FSM state coverage (ADV-017, A17-D11 to A17-D13) has these limits:
  - FSMs come only from the 40.4 pragmas, and each needs a `tool enum`
    parameter or localparam declaration listing its states; llg extracts
    neither FSMs nor states automatically (a state vector without states
    is an error);
  - a state vector is 1 to 64 bits;
  - the state expression of a concatenation has no VPI object
    (`vpiFsmStateExpression` is NULL with `LLG_VPI_UNSUPPORTED`);
  - transitions between states are counted and saved in the coverage
    database, but no 1800-2009 query reports them;
  - a concatenation's signals are watched one by one, so a time step that
    writes them in turn visits the intermediate values too (A17-D12);
  - pragma errors are reported when a model is built, not by `llg_ls`.

### Cause

Statement and toggle coverage are compiled in only where the source or the
VPI catalog asks for them, so other models are generated unchanged and pay
nothing; an always-available mode would cost every model a counter per
statement. The database stores counters by compile-time index under an
identity hash of the model's coverage metadata, which keeps loads exact and
atomic.

### Intended direction

No owner. The command-line switch exists (`--code-coverage`); the
other limits stay as documented unless a user needs them: UCIS or another
interchange format is out of scope, and assertion stepping (A17-D2) needs a
state numbering shared by all assertion engines.

### Reproduce

`tests/fixtures/sim/lrm_decisions/S46-D5_unavailable_kinds.sv`,
`S46-D6_string_scope_is_definition.sv`,
`S46-D14_toggle_coverage.sv`, `S46-D15_loaded_type_options.sv` (type
options), `A17-D2_control_callbacks.sv` (stepping),
`A17-D5_instance_callbacks.sv`, `A19-D59_sequence_and_property_operators.sv`,
`A17-D11_fsm_pragmas.sv` to `A17-D13_fsm_objects.sv`;
`tests/fixtures/sim/feature_completion/sim_046/db_errors.sv` (`-G W=3`
reads a database of the W = 4 model), `merge_foreign.sv` (statement and
FSM sections of another model), `option_kinds.sv` (`--code-coverage`);
`tests/fixtures/sim/feature_completion/adv_017/neg_fsm_width.sv` (a 65-bit
state vector), `relations.sv` (declarations without bodies).

## UVM example differences (U2)

**Status:** open. The UVM 1.1d and 1.2 `simple` and `integrated` examples were
run against the industry-standard simulator (U2, run again in U3 with the
recipe's `--timescale 1ns/1ns`); the differences fixed then are in
the [ki_uvm_run readme](../tests/fixtures/sim/feature_completion/ki_uvm_run/readme.md).
In U3 every run of both versions reaches its end with the industry-standard
simulator's verdict and error and fatal counts, except the UVM 1.2 codec
`hw_reset_test` run below; information and warning counts differ only where
random values (`ubus`) or the bus timing below (`codec`) differ. These remain.

### Mixed blocking and nonblocking assignments stop the run

**Symptom.** The `ubus`, `integrated/codec` and `registers/models/broadcast`
examples mix blocking and nonblocking assignments in one process. Lint rule
`mixed-assignments` (LINT-016) is an error by default, so `llg` stops before
generating the model; the industry-standard simulator runs them. With the
rule set to a warning (`[lint.rules.mixed-assignments] severity = "warning"`
in a `--config` file) generation continues.

**Cause.** Always-on lint with blocking errors is a product decision
(2026-10-06).

### `$typename` of a type parameter

**Symptom.** `$typename(T)` inside a class specialization returns the spelling
of the parameter where the frontend first met the type (`lib::registry::T`),
where the industry-standard simulator returns the resolved type
(`class tb.mine`).

**Cause.** The owned type table keeps one spelling per canonical type, the
first reference's; `$typename` reads it.

**Intended direction.** Capture the frontend's folded `$typename` text per
call, or a canonical spelling for every type kind as virtual interface types
now have.

**Reproduce.** `tests/fixtures/sim/feature_completion/ki_audit/typename_type_parameter.sv`
(its expected output pins the current spelling).

### Prefix increment of a member of a container element's object

**Symptom.** `v = ++t["a"].count["x"];`, where `t` is an associative array
of class handles and `count` an associative array property, stops generation
with "cannot resolve array base of select `count`". The statement forms
`t["a"].count["x"]++;` and plain reads and writes work.

**Intended direction.** Lower the prefix-increment expression through the
same element reference as the statement form.

**Reproduce.** `tests/fixtures/sim/feature_completion/ki_audit/neg_prefix_increment_container_member.sv`.

### Sequence declared in a clocking block

**Symptom.** `if (!sigs.mck.at_posedge.triggered) @(sigs.mck);`, where
`at_posedge` is a sequence declared inside clocking block `mck`, stops
generation with "concurrent assertions require one explicit signal clock".
The UVM APB agent (`integrated/apb/apb_master.sv`) uses it only inside a
branch selected by a tool's predefined macro; without that macro the agent
waits for an extra clock edge before each transfer, so the register examples
built on it (`integration/10direct`, `integration/20layered`,
`models/broadcast`) reach their final checks later than with the
industry-standard simulator, which takes its own branch. Their verdicts and
counts match.

**Cause.** Sequence and property declarations within a clocking block take
the block's clocking event (16.17); codegen requires an explicit signal clock
on the sequence itself.

**Intended direction.** Infer a sequence's clock from its enclosing clocking
block.

**Reproduce.** `tests/fixtures/sim/feature_completion/ki_audit/neg_clocking_block_sequence.sv`.

### The codec example's reset-jump run stalls after the jump (UVM 1.2)

**Symptom.** The UVM 1.2 `integrated/codec` `hw_reset_test` run (main phase
jumps back to reset) does not finish its second main phase: the objections
of `env` and the transmit sequence stay raised until the test's own
2 000 000 ns watchdog calls `$finish`. The industry-standard simulator ends
the phase, with one error of its own (a request of a killed register access
left on the bus sequencer). The other codec runs (1.1d both, 1.2 `test`)
pass; their counts of register-model race warnings (`UVM/FLD/SET/BSY`)
differ from the industry-standard simulator's (11 against none in both
`test` runs, 7 against 9 in the 1.1d `hw_reset_test` run), since the bus
timing below moves the register accesses.

**Cause.** Not the process control. The bus driver
(`integrated/apb/apb_master.sv`) waits one more clock per transfer unless
a tool-specific macro is defined, so each transfer takes 300 ns instead of 200 ns
and the transmit and receive interrupt threads of `tb_env` interleave
differently from the first main phase on: at the jump the industry-standard
simulator's transmit thread waits for a bus grant, while llg's threads are
idle. In the second main phase the receive thread clears its bit of `m_isr`
and sets it again in one process turn (`wait` statements that do not block
lie between the writes), so by decision T1-D11 the transmit thread's
`wait (!m_isr)` never sees zero. Under that decision's glitch coalescing a
level-sensitive `wait (expr)` is woken only at a check, which compares the
value with the one at the last check: a flag cleared and set again within one
process turn is no change, so a waiter for the cleared value misses it.

**Intended direction.** None: user decision 2026-11-19 keeps glitch
coalescing (T1-D11) with this consequence; the example's timing follows the
tool-specific macro.

## Semantic tokens around text macros (llg_ls)

**Status:** partially fixed by LS-MAC. Use sites, written arguments, open
buffers that use macros and syntax-broken buffers are served
([semantic token requirements](semantic_tokens.md)); the
gaps below remain.

### Symptom

- Attribute names (`(* keep *)`) get no token.
- An open buffer requested before its root's first analysis commits has no
  include context, so header macros are unknown there; a client without
  `semanticTokens.refreshSupport` keeps that stream until its next request
  (clients with it get a refresh, ST-50).

### Cause

The capture (`LexicalCapture` in `src/wrapper/slang_c_api.cpp`) gives
attribute names no kind. The isolated parse reads the include context of the
committed analysis only.

### Intended direction

Give attribute names a decorator kind.

### Reproduce

Request semantic tokens for a file with `(* keep *) logic a;`: the stream
has no token at `keep`.

## Semantic token kinds that need more capture (llg_ls)

**Status:** deferred by LS-TOK-9 (struct and union members and class
properties became `property` in LS-TOK-10); the colors are already right in
stock VS Code themes except where noted.

### Symptom

- Ports are `parameter` + `readonly` for every direction; only input ports
  are read-only.
- Subroutine arguments are `variable` and class methods `function` (clangd:
  `parameter`, `method`; same Dark+ colors).
- Modport, clocking-block, covergroup and constraint names are `variable`
  at their declarations; a covergroup is a type (`class`, `#4EC9B0`) in the
  analogue model. (Sequence, property and let names became `function` and
  their instances are bound since LS-TOKENS-13, ST-71.)
- At a let instance (`l(a, b)`, 11.13) neither the let name nor the names
  of its actual arguments get a token: Slang expands the let to its body
  expression and keeps no syntax of the call (the actuals carry the syntax
  of the formal references). Sequence and property instances are bound.
- In a sequence or property declaration that no assertion instantiates,
  member names after a select (`f` in `s.f`) and the local variables the
  declaration declares stay unbound: Slang creates neither without an
  instance. Instantiated declarations bind them through the instance.
- In the project stream (found by LS-TOK-10; the open-buffer stream was not
  checked), a use of a type parameter inside a module whose instance
  overrides it (`#(.T(logic [1:0]))`) is `type`, not `typeParameter` (same
  `#4EC9B0` in Dark+); the declaration stays `typeParameter`.
- A generate `case (expr)` condition has no reference token, and neither
  have the user names of a generate `if (expr)` condition (`D` in
  `if ($clog2(D) > 2)`, found by LS-BIF); `$name` calls there are tokenized
  from their spelling (ST-40).

### Cause

`TokenInfo` carries one lexical kind per token and these constructs share a
kind (`LexicalKind::Port`, `Variable`, `Function`, `Identifier`) in the
wrapper capture (`lexicalKindForSemantic` in `src/wrapper/slang_c_api.cpp`);
the encoder (`src/bin/llg_ls/semantic_tokens.rs`) sees no direction or
callable distinction.

### Intended direction

Carry the bound semantic detail (port direction, `FormalArgument`, class-scoped subroutine, covergroup)
into the token kind, bind sequence and property references, and keep the
symbol index and dump kind names unchanged.

### Reproduce

`llg_ls --dump-tokens` on a module with an output port, a class with a
method, and a named sequence referenced by an assertion.

## Unbound connection actuals count as unused (llg_ls, lint)

**Status:** found by LS-TOKENS-14, open. Their names have tokens and
navigation since LS-TOKENS-14 (ST-76).

### Symptom

A signal read only in the actual of a named connection to a port the
definition does not declare (`.bw_2(bw[2])` on a probe stub without `bw_2`,
Slang `PortDoesNotExist`) or of a repeated connection
(`DuplicatePortConnection`) is reported as unused (Slang `UnusedVariable`,
lint `unused-signal`), and an out-of-range constant select in such an actual
gets no `IndexOOB` or `out-of-range-select` diagnostic. The instance already
carries the port error.

### Cause

Slang never binds these actuals, so neither its analysis nor the owned DB
sees the reads. The capture binds their names for tokens and navigation
only (`bindUnboundConnectionNames` in `src/wrapper/slang_c_api.cpp`), after
the diagnostics are taken, and adds no semantic node.

### Intended direction

Leave the reads out while the port error stands (fixing the port removes
both findings), or capture the actual as a read-only use that the analysis
and lint count without lowering it.

### Reproduce

`module s(input logic a); endmodule module t; logic [7:0] b [2]; s u(.a(1'b0),
.x(b[5])); endmodule` in `llg_ls`: `b` is reported unused and `b[5]` is not
reported.

## Open-buffer token parse sees other files through packages and definitions only (llg_ls)

**Status:** partially fixed by LS-PKG, LS-PKG-2 and LS-TOKENS-13. Package items
used through any import form or `pkg::name` resolve in both serving paths,
import item names are references, definitions (modules, interfaces, programs,
checkers, primitives) the buffer names join the parse with their dependencies
(ST-75), and a stream served before a context change is refreshed
([token requirements](semantic_tokens.md));
the gaps below remain in the open-buffer path only.

### Symptom

- A package the buffer names only through text that the lexical scan cannot
  see (a `[compile] defines` macro or a header the buffer does not include
  expanding to `pkg::`, a package name built by token pasting) stays unknown
  in the open-buffer stream; its items get no token there.
- A definition the buffer names only through text the lexical scan cannot
  see (a `[compile] defines` macro or a header the buffer does not include
  expanding to the name, a name built by token pasting) stays unknown in the
  open-buffer stream. Unknown module instances are black boxes in both
  paths.
- A client without `workspace.semanticTokens.refreshSupport` keeps a stream
  served before a context change (the first analysis, a package file edit)
  until its next request; clients with it are refreshed (ST-50).

### Cause

The isolated parse (`features/requests.rs`) compiles the buffer with the
committed include context plus the units `PackageContext::sources_for`
(`features/package_context.rs`) selects by a lexical scan (`name ::` for
packages, any identifier spelling a declared definition) of the buffer, the
headers it includes and the selected units; names the scan cannot see are
not selected.

### Intended direction

Record, per committed unit, the packages and definitions Slang resolved for
it, and select dependency units from those facts plus the scan of the unsaved
text; send a semantic-token refresh after a commit that changes an open
buffer's context.

### Reproduce

Open a file with `` module top; `MK_INST endmodule `` where a `[compile]
defines` entry `MK_INST=ifc_x bus();` names an interface declared in another
project file and request `textDocument/semanticTokens/full`.

## Same-named definitions check one body (llg_ls)

**Status:** navigation fixed by LS-DEF-1: each body of a module, interface or
program that several files define navigates within itself
([behavior](config.md#lsp-duplicate-definitions)). Checks remain per
elaborated body.

### Symptom

- Without `[compile] top`, the body the frontend does not elaborate gets no
  semantic diagnostics (an undeclared name, a use before its declaration);
  only the `DuplicateDefinition` warning on the other definition's name
  points at it. Lint never reports findings in it.
- A named port or parameter label of an instantiation inside that body (as
  inside any uninstantiated body) has no definition target.

### Cause

Slang keeps one definition per name and elaborates only that one; with an
explicit top it also checks the others as unreferenced definitions, without
one it skips them. Capture adds the skipped bodies as uninstantiated
instances after the diagnostics are taken (`captureShadowedDefinitions` in
`src/wrapper/slang_c_api.cpp`), so they add no diagnostics, and Slang creates
only placeholders for instantiations inside an uninstantiated body.

### Intended direction

Check every same-named body as an unreferenced definition before the
diagnostics are taken, in the LSP only, so `llg` keeps compiling the selected
body alone, and bind placeholder labels to the ports of the selected child
definition.

### Reproduce

Two files that both define `module dup_top;` with
`assign done = undeclared_x;` in the first one: `llg_ls` reports only the
`DuplicateDefinition` warning in the second file.
