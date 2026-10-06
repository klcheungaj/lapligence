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
llg --gen-only --top tb --edition 2009 --out-dir <dir> \
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

`export byte limit exceeded` originates in llg's wrapper capture budget.
Raising that budget admits larger exports but does not reduce their memory
cost or bound total process RSS. A large design can still exhaust its export
budget, a record-count ceiling or available process memory.

### Intended direction

Lowering now sets the whole-run peak, so lower the DB plus typed IR it holds
(compact DB nodes with side tables for rare kind payloads), or import nodes
incrementally to shrink the staged snapshot further (the remaining 160 bytes
are mostly the 32-byte `Option<SourceRange>` and three 16-byte interned
texts). Rendering memory is tracked in the C emission entry below.
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
sharing threshold; this requires abstracting frame-type and PCA helper names
that are resolved after rendering. Stream the final text to `model.c` only if
the generated-model API stops returning it in memory. Generated `model.c` must
stay byte-identical, including both optimizer modes and the exact sharing
output; see the [emitter guide](../src/sim/emit_c/AGENTS.md#generation-memory).

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

## Remaining non-flattened fixed-value contexts

**Status:** open; RTL-002 and RTL-002b implement descriptor transport for
integral fixed arrays, and RTL-101/RTL-101b column layout for large records.

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
and tag checks on whole member copies. Column records still reject: a member
initializer that gives a column's cells different values (a column keeps one
element default); selecting a whole member array, a sub-record, or a `string`
or `chandle` member of a call result inside an expression, and comparing a
record pattern, conditional or tagged expression operand (only storage and
calls compare); an output or inout record argument with `real`, `string` or
`chandle` members of a function called inside an expression (statement calls
stage it through a temporary) and a `ref` formal of such a record; and a
whole-value binding in a continuous assignment. A nonblocking write of a whole
tagged-union member array checks the tag when it is issued; element writes
recheck at commit. The oversized source of a whole-value `matches` binding that
is a fixed array rather than a record still rejects (RTL-016).
Dense rows as descriptor pattern items and scatter targets, and dense arrays,
packed values, runtime `with` ranges and nested streams as parts of an
oversized stream, use descriptor transport (RTL-103); a resizable container operand of an oversized
stream rejects (SIM-020).

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
(megabytes at 5,000 cells). Correctness is unaffected; widening the
descriptor call ABI below the packed limit is the intended direction.

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


## Native record values outside by-value subroutine storage

**Status:** open; SIM-003 implements descriptor-backed native records for
subroutine formals, results and locals; SIM-006 stores them as elements of
queues, dynamic and associative arrays; SIM-007 adds one-dimensional fixed
arrays of strings, handles and native records, run-time indices into native
member arrays, packed-member selects, record-element equality and tagged
unions with native members in module/static variables, container members
of module records and of subroutine record values, native records, strings
and handles declared in procedural blocks, member defaults and any record
source as a module-record initializer, and member-wise equality of nested
records and member arrays.

### Symptom

Unpacked records with string, real, chandle or class-handle leaves copy,
compare and cross input/output/inout formals and results as runtime values,
are elements of resizable containers and of one-dimensional fixed arrays.
Queue, dynamic and associative members of records
(`struct { string s; int q[$]; }`) are their own containers in module and
static records and travel as companion containers of subroutine record values
(formals, results, automatic and static locals): methods, selects, `foreach`,
whole-record copies, ports, conditionals, equality (except associative
members) and assignment patterns work. These legal forms still reject with
explicit diagnostics: multidimensional fixed arrays of strings, handles or
native records; records with queue, dynamic or associative members as
container or fixed-array elements and in nonblocking assignments; class
properties of any unpacked record type with string, real, handle or container
members (these currently fail with a generic `has no resolved packed width`
diagnostic); equality of records with an associative member (and conditionals
with an ambiguous predicate on such records);
compound or nonblocking writes to a record element of a resizable container;
delayed (`#d`) nonblocking writes to a fixed array of native elements; a
run-time index into a native member array of more than 64 elements or with a
side-effecting index; a run-time index into an array of records nested in a
native record (`r.e[k].s`, which currently fails with a generic lowering
diagnostic rather than a dedicated one); a module, static or process-block
record as the actual of a native record `ref` formal (subroutine records
alias); nonblocking writes
to a static subroutine native record; fork-join_none capture of automatic
native records; `f(...).member` selects on a native result; native outputs
bound inside an expression (call them as a statement instead); record ports
whose type has a built-in semaphore, mailbox or process member (those handles
publish no change marker); an automatic record, string, chandle or class
handle declared in a procedural block that a `join_any` or `join_none` fork
can keep live while the block is entered again (declared in such a fork that
runs again, such as the `for (...) fork automatic string s = ...; join_none`
idiom, or in a block that runs again and starts such a fork reading it); a
member default whose value is itself a record with native members (its
frontend constant is not captured); a pattern item that is a nested native
record taken from a call result. Tagged unions with real, string, record or
class-handle members execute as module, static and subroutine values
(formals, results, locals, variable ports, conditional operators); as array
elements and in nonblocking writes they reject. String and real pattern variables bind
in process and subroutine bodies; handle bindings and structure patterns over
native records reject. A missing associative record element
compared with `==` reports the SV 7.8.6 warning once per member.

### Cause

A native value is one rooted `llg_value_t` tree; leaves are addressed by
constant item paths resolved at lowering time. Module-level native records keep
their per-member lowering, so whole-value transfers between the two
representations go leaf by leaf, and a run-time member index is a bounded
comparison chain over the declared leaves. Fixed arrays of native elements
are fixed-size views of the container runtime, so per-element delayed update
records and nested views are not modeled. A record member that is itself a
resizable container would need a companion container per record instance;
the native type descriptor has no queue bound or associative key to build
one from. A record, string or handle declared in a procedural block reuses
module storage, one copy per declaration and instance: an automatic one is
reset at each block entry, so a second live activation has no storage of its
own. The Db captures member defaults as constants, and an unpacked record
constant has no captured value. A native `ref` formal is the caller's
`llg_value_t` and companion containers, which module-like records do not have.

### Intended direction

Companion containers for container-valued record members; a native `ref`
formal passed as one reference per leaf (packed, string, real, handle and
container references already exist), so module-like and subroutine records
bind the same callee ABI (SIM-008); a root-plus-item-path pending record for static native roots
(a queued leaf pointer would dangle because a root replaces its leaves on
assignment); fork capture pins (SIM-010); Db capture of member access on
call results for `f().m`; native tagged unions in subroutine storage
through the same descriptors and root registry; and per-activation native
roots for forked automatic block records, strings and handles.

### Reproduce

`tests/fixtures/sim/feature_completion/sim_003/neg_native_*.sv`,
`sim_004/neg_static_native_record_nba.sv`,
`sim_008/neg_module_record_ref.sv`,
`sim_007/bad_member_select_limit.sv`, `sim_007/bad_block_record_*.sv`,
`sim_007/bad_block_native_*.sv` and `sim_007/bad_record_member_default.sv`.

## Resizable containers at subroutine, object and nesting boundaries

**Status:** open; SIM-006 implements containers as subroutine formals,
results and locals, procedural-block locals, class properties, record and
handle elements and nested containers.

### Symptom

These legal forms reject with explicit diagnostics: a `ref` container formal
whose actual is not a container variable of the formal's type; a fork branch reading an automatic container of the enclosing activation; an
instance container property selected through a handle (`h.q`) rather than
inside the class's own methods, and an initializer on such a property; a
container-result call used other than as a whole assignment source or a
statement (`f()[i]`, `f().size()`); a container argument in expression
position that is neither a variable of the formal's type nor a packed/real
assignment pattern; event controls and monitors on subroutine or object
containers; mutating methods of a nested container element (`q[i].push_back`;
`q[i].size()` works); a nested element written from a queue or associative
variable; `foreach` over a container of containers; and compound or nonblocking writes to a record element. Reads of a
missing nested associative element return the default without the SV 7.8.6
warning.

### Cause

Container operations name their storage by a container index: model storage
by a global, activation and object storage by a frame binding. A
handle-qualified property, a nested element and a call result have no binding
the operation can name without an addressed-container operand; references
need retained element cells and fork branches need capture pins. Nested
storage is a dynamic-array value, so queue methods on it would need dynamic
array forms of every queue mutation.

### Intended direction

An addressed-container operand (receiver or parent container plus index
path) for container statements and queries, receiver-qualified class
properties (SIM-011), retained element cells for `ref` (SIM-008), fork capture pins
(SIM-010).

### Reproduce

`tests/fixtures/sim/feature_completion/sim_006/neg_*.sv`.

## Real references and real-array expressions outside stable storage

**Status:** open; SIM-005 implements real/shortreal references to stable
storage and real fixed arrays across subroutine boundaries.

### Symptom

These legal forms reject with explicit diagnostics: a `ref` actual that is an
element of a real queue, dynamic array or associative array
(`ref actual ... names a real resizable-container element`); a real-array call
operand, equality or conditional arm that is a function call inside an
expression other than a whole-array assignment or another real-array input
(assign it to a variable first); a conditional real-array arm or row selector
that is not a plain variable read or constant; keyed (`with`) `min/max/unique`
and `sort/rsort` over real elements; and element-wise real-array expressions
above 4,096 elements (`REAL_ARRAY_ELEMENTWISE_LIMIT`). Whole-array storage
operands of any size pass by block copy.

### Cause

A real reference is the actual's `double*`, which a container reallocation
would invalidate; retained element cells need the pinning contract of
SIM-008. Real-array values that are not storage are lowered to one numeric
expression per element, evaluated where they are used, so operands that would
need a snapshot statement inside an expression are rejected rather than
evaluated twice.

### Intended direction

SIM-008 adds retained real container cells behind the same reference formal
ABI; a real-array value descriptor would let expression operands and larger
element-wise expressions run as cell loops.

### Reproduce

`tests/fixtures/sim/feature_completion/sim_005/neg_real_queue_ref.sv`.

## Streaming `with` targets outside the direct assignment path

**Status:** open; RTL-015 represents runtime and partly out-of-bounds fixed
`with` targets in a direct (blocking or nonblocking) streaming assignment, and
RTL-103 adds output copy-out from task and void-function call statements.

Such a range needs its bounds checked and its in-range elements written at run
time (IEEE 1800-2009 §11.4.14.4), which the `StreamAssign` statement does. A
copy-out runs it after the call with the selectors fixed when the call starts.
A function call inside an expression has no statement after it to run it in,
and an intra-assignment-delayed assignment lowers its target as a static
lvalue (SIM-014), so a runtime or out-of-bounds `with` range there rejects; a
constant in-bounds range works. Supporting the expression call needs a
writeback form of the checked unpack in the call ABI.

Three forms reject by owner policy rather than cost: a selector that reads a
target unpacked earlier by the same nonblocking unpack (nothing is published at
issue), by a right-to-left unpack (the consumed width must be known before the
bits are reordered) or by a copy-out (its selectors are fixed before the call).
Assign the length first in its own statement.

## Runtime-selected module reference connections have no qualified binding oracle

The fixed module-ref path requires constant actual selectors. A connection such
as `child c(values[index]);` rejects with an explicit runtime-reference-rewiring
diagnostic; it must not silently reread `index` on each access or invent a
canonical storage cell. IEEE 1800-2009 §23.3.3.2 describes hierarchical reference
binding, but the retained runtime-selector characterization has no adjudicated
binding/rebinding oracle. Qualify that boundary before enabling runtime-selected
connections. Static selected connections and nested packed projections execute.

## Delayed and event-controlled native writes

**Status:** open; SIM-004 queues untimed and `#delay` nonblocking writes and
drives zero-delay continuous assignments of strings and string records.

### Symptom

These legal forms reject with explicit diagnostics: a delayed continuous
assignment to a string or string record (`assign #1 s = t;`, SV 10.3.3), and
event or repeat intra-assignment timing on a string target (`s <= @(e) t;`,
`s = repeat (2) @(e) t;`). A blocking `#delay` assignment of a record with a
conditional source also rejects.

### Cause

A delayed continuous driver keeps an inertial pending value per driver, and
event-controlled NBAs run a detached waiter that captures the value. Both
records hold packed (`sv4_t`) payloads only; neither owns a string.

### Intended direction

Give the inertial driver and the detached event waiter an owned native payload
(string or chandle) next to the packed one, reusing the `llg_nba_t` native
member layout and its destroy path.

### Reproduce

`tests/fixtures/sim/feature_completion/sim_004/neg_delayed_string_continuous.sv`;
`module tb; string s; event e; initial begin s <= @(e) "x"; ->e; end endmodule`.

## `%l` in runtime-built format strings

Lowering replaces `%l`/`%L` with the scope's static `library.cell` binding in
every literal format (V §13.6; SV §33.7). A `$sformat`/`$sformatf` whose format
is a string value built at run time is interpreted by the runtime formatter,
which has no library table and still prints `work.<scope>`. Fixing it means
passing the binding next to the scope string through the runtime formatting
entry points. Reproduce with `string f = "%l"; $display("%s", $sformatf(f));`
inside a cell bound from a non-`work` library.

## Effectful helpers in runtime-callback evaluators

**Status:** narrowed by RTL-007b; two list forms remain.

Legal helpers with visible writes, persistent static state or descriptor-array
formals now run in processes for blocking, intra-assignment, nonblocking and
`->>` event controls, force sources and named-event/real event lists, and in a
private Postponed evaluation for `$monitor`/`$strobe` (writes outside the
helper reject there by SV 4.4.2.9). Still rejected: a named event in the same
list as such a helper whose sensitivity includes unpacked-array, container or
string storage (the atomic mixed wait has no source form for those markers),
and event handles other than declared named events in such a list or as the
target of such a `->>`. Stores a `$monitor`/`$strobe` helper makes to its own
static storage publish no event, so a hierarchical wait on that storage does
not wake.

## Event controls on subroutine storage in expanded tasks

**Status:** open (SIM-009).

### Symptom

A task whose event control (`@(...)`) reads one of its own by-value formals,
locals, or string or handle `ref` formals is expanded at each call site. These
legal forms still reject: such a task that also has a native record or
container formal ("needs caller-environment expansion, which is not
supported"); a recursive such task; and `@(t)` on a string `ref` formal
("cannot resolve signal reference"). `wait (cond)` over formals, event
controls on module signals and on `ref` formals with module-signal actuals,
and event formals of every direction take the typed call path.

### Cause

The typed body's evaluated event callbacks capture only process-block
automatics; a shared task body has no private context for its own formals
and locals, and no change marker for a string or handle `ref` actual, so
call-site expansion supplies them. Expansion cannot carry native formals and
cannot recurse.

### Intended direction

Capture the by-value formals and locals an event expression reads into the
evaluator's private context in the typed body, as process blocks do, and bind
string and handle `ref` actuals' change markers per specialization like
packed `ref` formals.

### Reproduce

`tests/fixtures/sim/feature_completion/sim_009/neg_event_local_native.sv`;
`task automatic down(int n); logic l = 0; fork #1 l = 1; join_none @(posedge l); if (n > 0) down(n - 1); endtask`;
`task automatic watch(ref string t); @(t); endtask`.

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

The intended direction for driven cells is a descriptor-backed net-array cell
table with a loop over a contiguous RHS view, keeping per-cell resolution
state but not per-cell generated code; see the remaining steps in
"Declared net views retain per-bit lowering bindings".

## Gate and UDP array elements emit one process function each

**Status:** open; found by RTL-020.

Each element of a gate or UDP instance array is lowered to its own
continuous process, and each process gets its own C function because the
elements read and write different constant bits, so exact body sharing does
not merge them. A 4,096-element mux UDP array produces 9.8 MB of `model.c`
(about 2.4 KB per element) after RTL-020's dense-index lookup, which already
removed the per-evaluation input copies (15.8 MB before). Evaluation cost per
element is constant; the cost is C size and compile time, linear in the
element count.

Reproduce with `mux3 m[4095:0] (y, sel, a, b);` over 4,096-bit vectors and
`--gen-only`, then measure `model.c`.

The intended direction is one shared process body per array declaration,
parameterized by the element's bit offsets, like the table-driven
procedural-continuous-assignment batches.

## Release does not restore a variable's continuous driver

**Status:** open; found by RTL-013, owned by the force/release feature.

IEEE 1800-2009 10.6.2: releasing a variable that is also driven by a continuous
assignment re-establishes that assignment. The generated model keeps the forced
value instead, both in ordinary `always` and in `always_ff`, until the
continuous driver's operands next change. Nets are not affected: a released
net recomputes its resolution immediately.

Reproduce with `logic [7:0] r, src; assign r = src;`, a process that runs
`force r = 8'haa;` and later `release r;`, and a `$display` of `r` after the
release without changing `src`; the model prints `aa` instead of `src`.

## Delayed enable gates drive X instead of L/H

**Status:** open (RTL-012 deferral).

An undelayed `bufif0/1` or `notif0/1` output is split into a strength0-only and
a strength1-only contribution, so an unknown enable drives L or H (IEEE
1364-2001 7.4, 7.10.2) and, for example, `bufif1 (w, 1'b0, 1'bx)` with a
`pulldown (w)` resolves to `650`/0. A gate with a delay keeps one contribution:
splitting would turn a 0-to-1 output into a turn-off of one slot and a rise of
the other, so the net could pass through Z and use the wrong transition delay.
The delayed gate therefore drives X (StX) for an unknown enable, and the same
pulldown example resolves to x. Supporting it needs one inertial handle that
publishes both halves with the transition delay of the combined output.

Reproduce with `bufif1 #1 (w, d, e); pulldown (w);`, `d = 0; e = 1'bx;` and
`#2 $display("%v", w);` (prints `StX`; the LRM result is `650`).

## Operator-overload update values on oversized or native targets

**Status:** RTL-017 and RTL-104 execute fixed operator overloads (SV §11.11),
including increment values, once-evaluated targets, relational expected types
and package overloads. One target class is left.

### Symptom

An overloaded increment or compound assignment whose value is used
(`y = x++;`, `y = (x += b);`) or whose target selector has side effects
(`a[next()]++;`) reports "an overloaded operator update whose value is used or
whose target selector has side effects requires a target within the
1048575-bit packed value limit without native members" when the target is
wider than the packed value limit (a 65,537-element `int` array) or a record
with a string, real or other native member. Statement forms with
side-effect-free targets (`x++;`, `x += b;`, and the same updates as `for`
steps) run for every target.

### Cause

These forms lower to the packed `Mutation` expression, which captures the
target's current value once. Descriptor-backed fixed values and native records
have no such capture: the statement forms re-read the target as an ordinary
call argument instead.

### Direction

Give `IrFixedValue` and native record roots a once-resolved read/modify/write
form whose old value can be kept.

### Reproduce

`tests/fixtures/sim/feature_completion/rtl_104/limit_native_value.sv`.

## Strict 2001 profile checks grammar and listed semantics

The pinned frontend parses 1364-2001 sources with its 1364-2005/SystemVerilog
grammar. The wrapper's `Verilog2001SyntaxProfile` closes the grammar side: a
Verilog-2001 compile rejects every parsed syntax kind outside the IEEE
1364-2001 Annex A allowlist and the listed optional parts of admitted kinds,
on the macro-expanded trees. [`editions.rs`](../src/core/compile/editions.rs)
adds keywords, system names and the semantic rules in the
[edition table](sim_features.md#target-language-editions), including variable
drivers. A 2001 constraint that is neither grammar nor one of those rules
stays admitted unless the frontend rejects it, for example an `output real` module port. Navigation snapshots have no subroutine bodies, so the
body rules do not run there. Directives in trivia are checked by
token, not by the syntax profile. Reproduce: compile `module tb(output real r); initial r = 1.0; endmodule`
with `--edition 2001`.

## Modport expression ports through virtual interfaces

**Status:** open (RTL-102 deferral).

A modport expression port `.p(expr)` (SV 25.5.4) executes through static
interface ports and hierarchical references, but a virtual interface handle
has runtime member slots only for interface storage, so `vif.p` rejects with
"modport expression port `p` is not supported through virtual interface view".
Supporting it needs a per-descriptor evaluator (and, for outputs, a writer)
for each expression port, selected by the bound instance at run time.

Reproduce with `interface i; logic [7:0] a; modport m(input .p(a[3:0]));
endinterface`, `virtual i.m v = inst;` and `$display("%h", v.p);`.

## Tagged-union members of packed-array elements

**Status:** open (boundary kept when member access through packed-array
elements was admitted).

A member of a tagged union that is an element of a packed array, such as
`tp[i].a` for `t_t [1:0] tp` with `typedef union tagged packed {...} t_t`,
rejects with ``tagged-union member `a` of a packed-array element is not
supported``. Reading or writing it needs the tag check on the selected
element; the packed element-member projection (`collection/packed_elements.rs`)
carries no tag guard, so it rejects rather than read inactive payload bits.
Other packed structure and union members of packed-array elements work, as do
tagged-union members of whole signals. Lowering it as an
`IrExprKind::TaggedSelect` / `IrLhs::TaggedSelect` whose steps are the element
chain followed by the guarded member step would admit it. Until then, select
the element into a tagged-union variable first.

Reproduce with `typedef union tagged packed { logic [3:0] a, b; } t_t;
t_t [1:0] tp; initial $display("%h", tp[0].a);`.
