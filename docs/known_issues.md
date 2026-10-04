# Known issues

Open design limitations that are understood but deliberately deferred. Each
entry states the symptom, the cause, the intended direction and how to
reproduce it. Remove an entry when the fix lands.

## Declared net views retain per-bit lowering bindings

**Status:** electrical groups, continuous writes and generated declared-view
descriptors are range-based; the lowering-side alias graph and binding list
are still per bit. RTL-011 judged the full fix out of its scope (see Cause).

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
names and observation/dependency behavior.

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

## Remaining non-flattened fixed-value contexts

**Status:** open; RTL-002 and RTL-002b implement descriptor transport for
integral fixed arrays.

Integral variable arrays copy, compare, select rows, merge conditionals, stream
(including multiple segments and unaligned slices), initialize and pass through
input/output/inout/ref formals and returns of static, automatic and recursive
functions without becoming one packed value. Array-valued pattern items and
pattern-lvalue row scatter use the same views. Oversized unpacked records,
arrays of records and finite tagged unions still have no descriptor layout and
retain the packed payload limit; so does the source of a whole-value `matches`
wildcard or binding, which rejects with its size (RTL-016). Descriptor pattern items and scatter targets whose rows are small dense
arrays inside an oversized source, dense arrays and runtime `with` ranges as parts
of an oversized stream, and nested oversized streams reject with explicit
diagnostics; constant in-bounds `with` ranges stream as sliced views (RTL-015).
Extending those paths through per-cell source expansion would recreate the
capacity cost.

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


## Streaming `with` targets outside the direct assignment path

**Status:** open; RTL-015 represents runtime and partly out-of-bounds fixed
`with` targets only in a direct (blocking or nonblocking) streaming assignment.

Such a range needs its bounds checked and its in-range elements written at run
time (IEEE 1800-2009 §11.4.14.4), which the `StreamAssign` statement does. An
output or inout copy-out actual and an intra-assignment-delayed assignment lower
their target as a static lvalue instead, so a runtime or out-of-bounds `with`
range there rejects with "requires a direct streaming assignment"; a constant
in-bounds range works. A runtime range over a record member array, ref formal or
local whose elements mix two-state and four-state members also rejects, because
its packed element write cannot apply member-wise state conversion; a uniform
element domain, any model array and every source use work. Supporting either
needs a copy-out/delayed stream plan or a member-wise conversion mask.

Two forms reject by owner policy rather than cost: a selector that reads a target
unpacked earlier by the same nonblocking unpack (nothing is published at issue)
or by a right-to-left unpack (the consumed width must be known before the bits
are reordered). Assign the length first in its own statement.

## Runtime-selected module reference connections have no qualified binding oracle

The fixed module-ref path requires constant actual selectors. A connection such
as `child c(values[index]);` rejects with an explicit runtime-reference-rewiring
diagnostic; it must not silently reread `index` on each access or invent a
canonical storage cell. IEEE 1800-2009 §23.3.3.2 describes hierarchical reference
binding, but the retained runtime-selector characterization has no adjudicated
binding/rebinding oracle. Qualify that boundary before enabling runtime-selected
connections. Static selected connections and nested packed projections execute.

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

## Oversized net arrays emit per-cell electrical code

**Status:** open; RTL-010 removed the quadratic net-array driver discovery.

Net arrays are not descriptor-backed: every cell owns an electrical group, a
declared-view alias row and an observation cell, and a whole-array continuous
driver gathers its RHS cells into one packed value with one element read per
cell. Generated `model.c` therefore grows by about 2.4 KB per cell (19.8 MB for
an 8,192-cell `wire [7:0] n[8192]; assign n = src;`, 39.8 MB at 16,384 cells),
and a 65,537-cell net array is impractical to compile. Lowering is now linear
in the cell count: driver sources are indexed by the cells they drive once,
instead of rescanning every source for every cell (4,096 cells: 9.2 s to
0.04 s of publication time on a quick build).

A whole net-array inout port behaves the same way. For
`child u(n)` with `inout wire [7:0] c [0:N-1]` and one driver on each side,
`--gen-only` on a quick build took 1.8 s / 8.6 MB of `model.c` at 4,096 cells,
7.2 s / 34.8 MB at 16,384 and 28.0 s / 140.5 MB at 65,537 (RTL-011
measurement): linear, never flattened into one packed value, but still one
electrical group per cell, because the partitioner extends runs only within
one cell.

Input-port links into a net-array formal and `unconnected_drive` pulls on one
(RTL-012) gather the cells into one packed contribution the same way. Above
the packed width limit they keep the older per-element storage write, which
bypasses the formal's internal drivers.

Gate and UDP outputs on net-array cells use the same per-cell groups (RTL-020:
a UDP driving one bit of a 200,000-cell `wire [7:0]` array generated 328 MB of
`model.c` and did not finish compiling in 500 s); variable arrays connected to
UDP inputs use descriptor storage and stay small.

The intended direction is a descriptor-backed net-array cell table with a
loop over a contiguous RHS view, keeping per-cell resolution state but not
per-cell generated code.

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

## Operator-overload increment values and expected types

**Status:** RTL-017 executes fixed operator overloads (SV §11.11); these
legal forms are rejected with specific diagnostics or remain unadmitted.

### Symptom

- `y = x++;` with an overloaded `++` reports "the value of an overloaded
  postfix '++' cannot be used". Statement and `for`-step forms run.
- `y = ++x;` on an unpacked operand fails in lowering with "assignment-like
  expression to a streaming target", the existing limit for any unpacked
  assignment used as a value (`y = (x = z);` fails the same way).
- `arr[next()] += b;` with an overloaded `+` reports that the target "is read
  and written separately and must not have side effects".
- Overloads differing only in result type need a cast inside a relational
  operand even when the other operand fixes the comparison type.
- An overload declared in a package is not visible through `import`.

### Cause

The frontend builds `x = f(x)` for increments and `A = op(A, B)` for compound
assignments from ordinary call and assignment nodes, re-binding the target as
an operand. No owned node yields an old value or binds the target once for
both uses. Expected types are threaded through assignment-like contexts only.
Overload declarations are unnamed members, so wildcard imports cannot carry them.

### Direction

Add an owned mutation form whose value can be the pre-update aggregate and
whose target selectors are frozen once, reuse it for compound overloads, and
pass the opposite operand's type as the expected type of relational operands.

### Reproduce

`tests/fixtures/sim/feature_completion/rtl_017/neg_postfix_value.sv` and
`neg_target_side_effects.sv`.

## Strict 2001 profile gates a listed set of keyword-free later forms

The pinned frontend parses 1364-2001 sources with its 1364-2005/SystemVerilog
grammar. [`editions.rs`](../src/core/compile/editions.rs) rejects later
keywords, system names and the keyword-free forms listed in the
[edition table](sim_features.md#target-language-editions), but not every
1364-2005 or SystemVerilog-only production in Annex A. Forms found later stay
admitted until a gate is added. Token-sequence rules (labels, `.name`, casts,
`[size]` and similar) skip tokens produced by macro expansion, whose shared
use-site range has no source order. Navigation snapshots have no subroutine
bodies, so the body rules do not run there. The direction
is a Slang parse option for the 1364-2001 grammar, kept as a tracked patch.
Reproduce: compile a later form that is absent from that table, or an end
label produced by a macro, with `--edition 2001`.

## Frontend diagnostics do not show `` `line `` positions

Simulator diagnostics, assertion messages and coroutine site locations append
the `` `line ``-mapped position to the physical one. Slang diagnostics are
printed from `compile::Diag`, which has only the physical file, line and
column. Scope-based runtime locations (`$finish`, severity tasks) print a scope
path, not a file, and stay physical. Adding a logical field to `Diag` would
change its 57 struct-literal construction sites. Reproduce:
`tests/fixtures/sim/feature_completion/rtl_019/macro_error.sv` with a
`` `line `` directive before the macro use.
