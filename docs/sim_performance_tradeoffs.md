# Simulation speed versus model compile time

`llg` emits a C11 model per design and builds it with the platform C compiler,
so a design pays twice: once to compile the model, then on every simulation
run. This page records each generated-code and runtime choice that can affect
simulation speed, even only in theory, whether it trades against compile time
or memory: the decision and the measurement behind it. Compile-only changes,
such as compiler warnings, build parallelism and identifier length, are left
out.

Rules applied to every decision:

- Simulation output stays byte-identical.
- A compile-time gain must not slow simulation beyond measurement noise.
- Size thresholds are named constants chosen by measuring several designs,
  never one test's shape.

Measurements: Linux x86-64, GCC 14, compile time from
[`perf/scripts/compile_time.py`](../perf/scripts/compile_time.py), simulation
time from [`perf/scripts/corpus.sh`](../perf/scripts/corpus.sh) in alternating
A/B pairs (medians), profile shares from the SIGPROF sampler in
[`perf/README.md`](../perf/README.md). Design names below are configurations of
those scripts. Differences within the observed spread are reported as noise.

## Adopted

### C optimization level: `-O3`

- **Trade-off:** lower levels compile faster; higher levels simulate faster.
- **Decision:** `-O3` by default. `--model-opt-level <O0|O1|O2|O3|Os>`
  selects another level.
- **Evidence:** the sum of the five standard corpus simulation medians is
  39.2 s at O1, 29.6 s at O2 and 28.4 s at O3; `wide-values` is 20% faster at
  O3 than at O2. Large models build equally fast at O2 and O3
  (`many-registers-10k` 48.5 s at both). Only small models build faster at lower
  levels (`tasks`: 4.0 s at O2 against 6.7 s at O3, single cold builds), where
  both are seconds.

### Shared code for repeated instances

- **Trade-off:** one body per class of identical instances shrinks the C and
  its compile time. Each storage access then goes through an instance record
  (one extra load), and shared subroutines take a hidden argument.
- **Decision:** share classes with at least `LLG_SHARE_MIN_INSTANCES`
  instances (default 4; `unlimited` disables), in
  [`model/sharing.rs`](../src/sim/emit_c/model/sharing.rs).
- **Evidence:** at 4 instances the model compiles 41% faster (19% at 2), with
  simulation within noise. On the corpus, `tasks` simulates 10.8% faster and
  `many-masked-20k` 9.9% faster; the rest are within noise. Pairs and triples
  stay unshared because their gain is small.
- **Netlists:** cell bodies with specify paths, sequential UDPs and bit-select
  port links share too: their path sets, UDP states and selected bits are
  record operands. At 4,000 cells the model builds and runs in 5.1 instead of
  21.4 s (buffer chain), 6.4 instead of 24.7 s (hierarchical blocks) and 7.2
  instead of 147 s (inverters on bus bits), with identical output. Simulation of
  2,000-cell netlists with 5,000 input toggles is not slower (10.7-11.5 to
  9.7 s with path delays, 54.6 to 50.4 s on bus bits, three runs each).

### Per-instance netlist data as tables

- **Trade-off:** per-instance data (nets, path and timing-check sets,
  coroutine descriptors, spawns) emitted as initializer code makes the C, its
  compile time and the executable grow by several KB per cell. As table rows
  plus per-module templates, the runtime lays the structures out once at
  start (one loop per kind) and a shared body class has one descriptor.
- **Decision:** nets are `llg_net_shape_t` rows plus exact-size pools laid out
  by `llg_net_layout`; path and timing-check sets are per-module templates
  plus instance references (`llg_module_paths_layout`,
  `llg_timing_checks_layout`), and their condition evaluators take the
  instance record as context; a shared root class has one descriptor, one
  record array and `llg_model_spawn_class` row ranges; identical frame-size
  assertions are emitted once. Generate-iteration bodies of every kind, driver
  slots, SDF gate delay slots and per-site inertial handles are sharing
  operands. The structures the scheduler reads are unchanged, so the
  simulation fast paths are too.
- **Evidence:** `model.c` per cell 6.4-7.5 KB -> 0.5-0.75 KB (chain, hier and
  bus netlists at 4k-256k cells, 9.5-12.7x); 64k-cell gcc build 572 -> 10.5 s
  (chain) and 1,325 -> 13.3 s (hier), executable 374 -> 59 MB and 421 -> 76 MB,
  simulation 17.8 -> 14.9 s and 615 -> 602 s with identical output. Interleaved
  4k-cell and corpus simulations are within 2% (wide values +1.2%, half of
  it from the runtime build alone). `many-registers-10k`/`20k` compile in
  0.55/0.91 s instead of 2.2/4.8 s, unlike the rejected model-wide descriptor
  array, because only bodies of one shared class share a descriptor.

### Timing tasks as calls, not inline expansions

- **Trade-off:** inlining saves a call and frame per invocation; calls avoid one
  copy of the task body per call site.
- **Decision:** event, disable and timed tasks are called. Tasks with `ref`
  event formals are specialized per static actual instead of inlined.
- **Evidence:** on `testbench_tasks` N=128, `model.c` shrinks from 20.2 to
  5.8 MB and its compile from 108 to 27 s. `tasks` simulation drops from 2.48
  to 1.60 s, because the smaller code fits caches better. (That A/B also
  includes compact identifiers, which do not affect run time.)

### Ports sharing their source

- **Trade-off:** a port is a continuous assignment, so a copy process per
  port keeps both sides independent objects, but every change of a clock or
  other widely connected signal wakes one copy process per instance before
  any reader. Sharing storage removes those processes; the sink then shows
  the source's new value in the same evaluation.
- **Decision:** a port sink shares its source's root storage when that is
  what a merge would give: an input variable formal of a whole variable of
  the identical type (decision A21-D1), an output variable actual that only
  the port writes (A21-D3), and a `wire`/`tri`/`logic` net sink that only
  its port drives, merged with a variable or whole resolved net source of the
  same type (A21-D2). A net sink with other drivers or a dominating net type
  merges with its source into one resolved net instead (A21-D4), which costs
  one group and no process. Every other case where sharing would be
  observable keeps the copy (force/release on a variable sink, conversions,
  initializers, net delays, selected actuals, specify paths and timing
  checks, SDF, VPI models for storage aliases, a time-0 observer of a source
  that does not start at X), in
  [`collection/port_aliases.rs`](../src/sim/codegen/lowering/collection/port_aliases.rs),
  [`collection/nets/port_sinks.rs`](../src/sim/codegen/lowering/collection/nets/port_sinks.rs)
  and [`collection/nets/port_merges.rs`](../src/sim/codegen/lowering/collection/nets/port_merges.rs).
  An indexed part-select port actual whose index is a constant per generate
  iteration becomes a constant part-select whose bounds are record operands
  of the shared body.
- **Evidence:** on 64-instance port-select workloads the simulation is 38%
  (`lfsr[i % 28 +: 4]` actuals) and 43% (constant inputs) faster than with a
  copy per input (A21-D1). Net and output ports (A21-D2, A21-D3), pinned
  medians of 7 interleaved runs: a 256-stage `input wire`/`output wire`
  hierarchy (16-bit, depth 3) 14.5 s → 1.9 s (2304 links and 2304 of 2817
  net groups gone, `model.c` 494 → 167 KB); the same hierarchy with variable
  outputs 1.94 → 1.01 s (768 links gone, `model.c` 274 → 122 KB). The small
  designs lose all their links (cpu 21, fifo 15, uart 6) with times within
  noise; net-array element and part-select actuals keep theirs, and the
  corpus workloads, which have no instance ports, generate identical C. Merged net ports (A21-D4), pinned medians of 7 interleaved runs: 1,000
  generate instances, 20,000 cycles, each with a whole `wire` net into a
  child that also drives it 11.1 s → 8.5 s (−23%), and into a `tri0` formal
  7.7 s → 5.2 s (−32%); 1,000 links and 1,000 of 2,001 net groups gone,
  `model.c` 822 → 668 KB and 739 → 585 KB. The `perf/` corpus and designs
  generate identical C. Selected net actuals (A21-D5) merge bit by bit, plain ones too since
  merged formals became one-run net alias views (see "One-run net alias
  views" below). A net alias view refreshes only the
  changed net's parts (`llg_net_alias_ref`); rebuilding it from every part
  per bit change was quadratic (a 1,000-bit bus with coerced one-bit inputs,
  1,000 cycles: 24 s → 1.6 s). Pinned medians of 5 interleaved runs, 1,000
  instances, 20,000 cycles: inputs on whole `wire` net-array cells 13.0 s →
  10.3 s (−21%, 1,000 links gone, `model.c` 3.5 → 1.5 MB); coerced inputs on
  cells 19.8 → 18.9 s; coerced one-bit inputs of a bus driven per bit
  13.6 → 14.7 s (+8.5%) and of a bus with one wide driver 8.2 → 11.1 s
  (+37%), the cost of the merged nets the copies got wrong (since recovered
  by shared groups, below). Plain bus-bit
  inputs, outputs and gate netlists keep identical C (4.5 → 4.5 s), as do
  the `perf/` corpus and designs.

### Shared groups for split vectors

- **Trade-off:** when the bits of a vector net join different electrical
  roots (a selected port or inout per bit, a piecewise alias), one group per
  bit makes every wide driver of the vector write one slot per bit and every
  bit change refresh the vector's alias view; one shared group gives every
  per-bit member and driver a full-width slot of its own, so its slot memory
  and the cost of a write spanning it grow with members times width.
- **Decision:** `collection/net_partition.rs` keeps adjacent columns in one
  group while some member continues and every member and source covers one
  contiguous affine range, up to `MAX_SHARED_RUN_SLOT_BITS` (2^22 slot bits);
  a narrow source writes only its part of its slot (`IrLhs::Part`, shared
  bodies take the bounds as operands), a ranged publication refreshes only
  the alias parts it overlaps (`llg_net_alias_span`, from
  `LLG_NET_ALIAS_SPAN_MIN` = 16 entries), and a selected driver turning off
  keeps its interval node.
- **Evidence:** pinned medians of 5 interleaved runs, 1,000 instances,
  20,000 cycles (load average 3.5 → 2.9): coerced one-bit inputs on a bus with
  one wide driver 11.1 s (per-bit groups) → 7.95 s, below the 8.18 s of the
  old copies; driven per bit 15.0 s → 12.3 s (copies 13.5 s); `model.c`
  1.11 → 1.01 MB and 1.17 → 1.12 MB (copies 0.69 and 0.79 MB). Merging every
  plain selected port (a probe, 3 runs): a 1,000-bit bus of one-bit inputs
  4.49 s (copies) → 2.69 s, `model.c` 0.42 → 0.77 MB; a 256×8 nand netlist
  14.3 s → 9.2 s, `model.c` 0.93 → 2.89 MB. The `perf/` corpus and designs,
  gate netlists and plain selected ports generate identical C.

### One-run net alias views

- **Trade-off:** a merged formal is a declared view of resolved group bits.
  A view with its own storage, part array, descriptor, storage-table entries
  and start step cost about 440 B of C per formal against about 110 B for a
  copy's net and link, so merging every plain selected port, although faster,
  made `model.c` 1.8–3× larger. A view without storage costs a little
  runtime work at start (one loop) and is limited to views whose whole width
  is one run of one group slot.
- **Decision:** such a view (`IrSignal::net_view_part`) is a zeroed
  `llg_net_alias_t` plus one `llg_net_view_t` row of the model-wide
  `llg_net_views` table; `llg_net_views_start` binds it and starts it from its
  group, so it has no storage and no start step. Other views keep the old
  form. Plain selected ports now merge (A21-D5). Member slots of a bit-level
  group whose views are all one-run views of it are silent
  (`silent_slots`): a group with one source then resolves at once, as the
  source's own net did, instead of deferring like a multi-driver net (S2-D11),
  so a time-0 reader of a constant-driven vector still sees its value.
- **Evidence:** pinned medians of 5 interleaved runs, 20,000 cycles (load
  average 6.6 → 8.4; before = the old exact copies; `model.c` sizes and
  Release `-j1` compile times): a 1,000-bit bus of one-bit inputs 4.51 s →
  2.57 s, `model.c` 421 → 404 KB, compile 48.6 → 47.0 s, peak RSS 6.1 → 5.1 MB;
  a 256×8 nand netlist 21.0 s → 9.65 s, `model.c` 926 → 758 KB (merged with
  the old views: 2.89 MB), compile 43.8 → 43.9 s, peak RSS 15.6 → 9.0 MB.
  Designs that already merged get smaller with the same run time: coerced
  one-bit inputs on a bus driven per bit 12.6 → 12.5 s, `model.c` 1.12 →
  0.75 MB, compile 61.7 → 50.3 s; with one wide driver 8.05 → 8.03 s (7 runs),
  1.01 → 0.65 MB, compile 54.9 → 45.7 s. `perf/designs/gen16_proto.sv` and
  `gen64_proto.sv` now merge their selected ports (96.5 → 86.0 KB and 154 →
  144 KB); the rest of the `perf/` corpus and designs, the gate fixtures and
  `instance_sharing/netlist_cells.sv` generate identical C, and the
  bit-level net fixtures all shrink (for example `net_partition/ranges.sv`
  700 → 582 KB).

### Loops and tables instead of unrolled statements

Each item replaces per-element C with a table and a loop, sometimes calling a
non-inlined helper. The trade-off is loop control and indirection per element
against code whose size, and compile time, grows with the element count.

| Construct | Decision | Compile | Simulation |
| --- | --- | --- | --- |
| Net storage setup and teardown | One cell array per net; descriptor tables | `many-registers-10k` build 86 → 4 s | Startup only; corpus within noise |
| Combinational UDP | One truth table per primitive, evaluated by `sv4_udp_eval` | UDP model GCC 32 → 0.2 s; 1.7 MB → 50 kB of C | 15% faster |
| Combinational UDP lookup | Dense `3^n` output index per definition up to 10 inputs; inputs read in place | 4,096-instance mux array: 15.8 → 9.8 MB of C | 2,000 input steps: legacy about 24 → 13 s, compact 9.2 → 5.5 s (shared host) |
| Repeated identical concatenation operands | Loop from `REPEAT_VALUE_MIN_COUNT` (4) repeats | deep-defaults GCC 43 → 4 s | 2.7 → 1.9 ms |
| Whole unpacked-array values (copy, compare, `?:` merge, formals, NBA, continuous assignment, `%p`) | One runtime call per run of at least `LLG_DENSE_CELL_RUN_MIN` (6) consecutive cells, in dense or descriptor storage | 256 elements: up to 1.24 MB → 24 kB of C, GCC up to 83 → 0.2 s and 2.3 GB → 41 MB; 4,096 elements: 10-19 MB → 14-22 kB, and the per-element model did not finish compiling at `-O3` in 3 hours; 5,000-element task and `%p` fixture 17.3 MB → 41 kB | 144 pairs at 16 and 256 elements: median 0.26× (0.03-0.26× for whole-value operations at 256); 4,096-record copy at `-O1`: 7.27 → 0.93 s |
| Wide periodic constants (whole-array defaults) | One period, replicated at startup, from `PERIODIC_CONSTANT_MIN_BITS` (4,096) bits and 4 repeats | 4,096-record formal default: 60 kB → under 1 kB of C | Startup only |

The repeated-operand threshold is 4 because shorter runs gained at most a
few percent of compile time. Below the threshold, emission is unchanged.

The cell-run threshold is 6 cells, from 90 pairs of per-element code against
range calls (8-bit cells, 36-bit records and nested records; copy, `?:`
merge, `==`, task formals and default initialization; 4 to 16 elements).
From 6 cells every pair was faster (at most 0.72×, median 0.56×; 0.38× at
16). At 4 and 5 cells the medians were 0.65× and 0.62×, but copying 4
records and initializing 5 nested records measured 1.16× and 1.19× in single
pairs, so shorter runs keep per-element code, which stays bounded by the
threshold. The range calls gather a run into limb words and build the value
once: inserting cell by cell renormalized the whole wide value per cell,
which made a 4,096-record copy quadratic (1.7× slower than per-element
code at `-O1` instead of 0.13×).

### Long bodies as helper functions

- **Trade-off:** GCC and Clang optimize a function in time superlinear in its
  size, so one C function per long body compiles slowly; a helper function per
  statement run adds one call and an exit test per run.
- **Decision:** statement runs that share no state with the rest of the body
  are moved into static helpers of at most `LLG_OUTLINE_RUN_WEIGHT` (1,024)
  weight units: IR nodes, plus two per formatting argument and two per block
  scope for the fixed C they render, so a unit is one to two C lines in
  every statement form and helpers stay at about 700-1,800 lines; runs under `LLG_OUTLINE_MIN_RUN_WEIGHT` (128) stay inline, so
  bodies up to one run keep their C byte for byte
  ([emitter notes](../src/sim/emit_c/evaluation.md),
  [residual cases](known_issues.md#long-bodies-that-cannot-be-outlined-compile-superlinearly)).
- **Evidence:** `--no-opt` `rtl_006` constant-operand bodies repeated 1, 2, 4
  and 8 times in one `initial` block (9,000 to 70,000 lines of C), one CPU:
  GCC 5.7, 13.5, 38.6 and 129.6 s become 2.1, 3.9, 8.1 and 14.1 s; Clang 19
  3.0, 10.2, 43.2 and 196.6 s become 1.7, 3.4, 7.7 and 16.9 s. Weights 256
  to 4,096 were measured on the same series and on 3,000 distinct
  assignments: 4,096 was slower throughout and 2,048 below 8 repeats, and 256
  and 512 were no faster on distinct statements (Clang 2.7 s against 1.6 s)
  while growing the C by 3-9%. Their gain at 4 and 8 repeats came from GCC
  folding identical helpers of the repeated text, which real bodies rarely
  contain. Across 2,665 test fixtures per optimizer mode, 44 (default) and
  52 (`--no-opt`) models change, 6% smaller (block locals move from the
  coroutine frame into helper C locals), and the rest are byte-identical, as
  are the `perf/` corpus and `perf-portsel` models. Hot loops that do split (400
  assignments per cycle in 3 helpers; a 16-arm `case` of 60 assignments per
  arm in 16 helpers) ran 1.006× and 1.008× the inline time, within the
  spread of 7 interleaved pinned pairs.
- **Evidence (weight units):** counting IR nodes alone, a `$display` of 17
  folded constants weighed 19 and rendered about 90 lines, so the optimized
  `rtl_006` constant rows became a 4,610-line helper, which MSVC `/O2` did
  not compile within 180 s on Windows CI (a 2,900-line helper did). With
  the per-argument and per-scope units, and constant arguments built in
  their argument entry, the largest helper is 1,284 lines (largest of any
  test fixture model: 1,777 lines; before, 68 fixture models in the two
  modes had a helper over 2,000 lines), and GCC 14 `-O2` (the local stand-in for MSVC) builds the model
  in 0.6-0.9 s instead of 1.4 s. Run weights 768 to 2,048 were measured
  again on those rows, the 1x-8x series, 3,000 distinct assignments and a
  mixed-type fixture (GCC 14 and Clang 19, `-O2`/`-O3`, one pinned CPU, host
  load about 20): no setting was best on every input, 768 was faster with
  GCC on several inputs but 2.3× slower with Clang on the distinct
  assignments (`--no-opt`), as 512 was before, so 1,024 stays. Generated
  C changes only in long bodies and in formatting calls with constant
  arguments; `perf/` hot loops keep their C.

### Coroutine frame: C locals wherever possible

- **Trade-off:** keeping every local in the coroutine frame is simplest and
  survives any suspension. But every access goes through memory, and the large
  frames slow GCC. C locals are faster to compile and run, but cannot live
  across a suspension.
- **Decision:** a declaration is a frame field only if its scope contains a
  resume point. Resume-stable frame fields that are read often are cached in C
  locals and reloaded after each suspension.
- **Evidence:** on a process without resume points but with 10,747 frame
  accesses, keeping everything in the frame doubled `model.c` and raised compile
  from 7 s to 15–19 s; scope narrowing removed both costs. Caching cut the
  `testbench_tasks` N=128 compile by 17%. Simulation was not timed separately;
  both changes only remove memory accesses, and output was identical.

### Cancellation checks only where cancellation can change

- **Trade-off:** a check after every statement is simple and conservative;
  each check costs code and time.
- **Decision:** check only after resume points and after operations that may
  disable the activation.
- **Evidence:** `testbench_tasks` N=128 checks fall from 44,631 to 8,222, and
  the compile from removing them drops 20%. Semantic oracles cover cancellation
  at call sites and scope exits.

### Wait inputs as compound literals

- **Trade-off:** event-spec arrays passed to a wait can live in the coroutine
  frame, filled by a copy before each wait, or be built in place as a C
  compound literal at the wait site. Literals drop the frame copy and shrink
  frames, but are rebuilt at every wait.
- **Decision:** compound literals. The runtime copies wait inputs during the
  call and keeps no pointer to them, so the literal may end with the call.
- **Evidence:** compile 2–6% faster, frames 4% smaller, and frame copies on
  `testbench_tasks` N=128 fall from 9,217 to 0. Simulation was not timed
  separately; output was identical.

### Indexed scheduler bookkeeping

Linear scans in the scheduler dominated simulation time. Indexes and intrusive
links make each operation independent of the total number of waiters,
processes or clock edges, at the cost of a few bytes per object.

| Structure | Decision | Simulation | Memory |
| --- | --- | --- | --- |
| Signal-change publication | Per-dependency waiter index instead of scanning every waiter; reverse-registration wake order kept | `zero-delay` 20× faster (31× at 20k), `many-registers-10k` and `many-masked-20k` 4.2×; one publish costs ~100 ns at 10k and 20k waiters (was 26 and 57 µs) | Process record 376 → 400 B, wait +24 B, activation +16 B, handle +8 B, 32 B per subscription; peak RSS about +1% |
| Waiter, process-handle and activation removal; process registration | Intrusive back-links and a free-slot bitmap: O(1) instead of list searches | Included above (these were 22–48% of samples) | Included above |
| Clock-history lookup | Exact-pointer hash index; entries expire at descriptor release and time-slot advance | `tasks` 75% faster, suite fixture 34%; lookup share 69% → 0.03% | 8 B per hash bucket (about 11–21 B per live entry); history holds only current-slot entries |
| Fork groups and named-event waiters | Tail pointers and back-links: O(1) fork-group append/unlink and event-waiter removal | One parent spawning 300k `join_none` children: 2,321 → 0.41 s; 1M children 1.34 s (previously over an hour); corpus within noise | Process record 400 → 424 B, fork group 72 → 88 B, 4 B per entry of a multi-event wait list; ~786 B per process at 1M |

### Inline storage for common waits

- **Trade-off:** storing every wait's sensitivity list and value snapshot on
  the heap keeps one simple representation, but costs several allocations per
  wait. Inline storage avoids them, but every process record pays for the
  inline space, and the runtime carries two wait representations.
- **Decision:** one signal of up to 64 bits (`LLG_WAIT_INLINE_SPECS` = 1,
  `LLG_WAIT_INLINE_LIMBS` = 1) or one named event is stored inside the wait
  record. Wider, mixed and expression waits stay on the heap. Timed-queue links
  moved into the wait's payload union to make room, so neither the wait
  (128 B) nor the process record (400 B) grows.
- **Evidence:** a one-signal edge wait went from 5 allocations to 0, and a
  named-event wait from 1 to 0. Simulation: `many-registers-10k` 42% faster,
  `many-masked-20k` 28%, `zero-delay` 27%, the suite fixture 4%; `tasks` and
  `wide-values` within noise. Peak RSS fell slightly (`many-registers-10k`
  65.5 → 63.6 MB). Every measured clocked wait fit the inline capacity, so
  larger capacity would only add bytes per process.

### Frame-resident value cells

- **Trade-off:** a heap-allocated value scope per local per activation keeps
  every cell's lifetime independent of its frame. Putting the cell in the
  coroutine frame avoids the allocation, but enlarges the frame, and is only
  safe if nothing can hold the cell after the frame is reused.
- **Decision:** packed, real and string locals proven not to escape get their
  ownership node and cell in the frame. Anything that may outlive the frame
  keeps the heap path: NBA targets, `ref`/`output`/`inout` actuals, fork
  captures, waits on the local, clocking, VPI/DPI, monitor/strobe readers and
  any operation the analysis cannot prove safe. Process ABI 2 → 3.
- **Evidence:** `tasks` scope-storage allocations fell 9.4%. On a pinned A/B,
  `zero-delay` ran 7.5% faster and `tasks` and `many-masked-20k` 2.2% faster;
  the rest were within noise. Each eligible local adds 112 B to its frame (a
  task root frame grows from 1,640 to 2,760 B). Designs without eligible
  locals are unchanged.

### Bounded retention of finished work

- **Trade-off:** keeping completed process records and every sampled slot until
  teardown needs no lifetime tracking, but memory then grows with simulated
  time. Releasing them costs a few checks at completion and 8–16 B per sampled
  history node or value-only sampled item for list links and depth.
- **Decision:** completed top-level processes (assertion actions, detached
  spawns) are freed at the next reap boundary; assertion reads register
  signals value-only; clocking sources and `$past`/`$rose` domains keep only
  the depth their readers declare (largest input skew, largest `$past` ticks).
- **Evidence:** `perf/values/assertions.sv` (32 lanes), median of 3 wall
  times and heaptrack requested peak heap, identical output:

  | Backend | Cycles | Peak heap before → after | Wall s before → after |
  | --- | ---: | --- | --- |
  | legacy | 200 / 1500 / 6000 | 25.8 M / 189 M / 756 M → 533 K / 538 K / 536 K | 0.42 / 3.32 / 15.33 → 0.28 / 2.08 / 8.33 |
  | compact portable | 200 / 1500 / 6000 | 19.9 M / 146 M / 584 M → 487 K / 490 K / 487 K | 0.32 / 2.67 / 10.93 → 0.22 / 1.85 / 6.73 |
  | compact GMP | 200 / 1500 / 6000 | 19.9 M / 146 M / 584 M → 487 K / 487 K / 487 K | 0.32 / 2.71 / 10.43 → 0.22 / 1.71 / 6.80 |

  Peak RSS at 6000 cycles fell from 851 / 630 / 630 MiB to about 3 MiB. The
  time gain was not profiled; the removed work is a history node per signal
  per slot, and a search of the 324-entry history list on every signal write.

### Models with the VPI object catalog

- **Trade-off:** a model built with `--vpi` or `--dpi-lib` keeps everything an
  application may reach: the object catalog, port connections that other
  models share or merge, writable gate delays, stopped statement and toggle
  coverage, and, once an application runs, a frame record per framed call.
  The catalog costs compile time, memory and start-up; the kept structure
  costs run time only where it is on the hot path.
- **Decision:** catalog models link position-dependent on Linux with GCC or
  Clang (`VPI_CATALOG_LINK` in [`build.rs`](../src/sim/build.rs)), so their
  catalog pointers need no load-time relocation, and the catalog is
  validated and indexed on the first VPI use that can reach a design object,
  not at a registration ([I/O, VPI](../src/sim/rt/io.md)). Kept on purpose:
  port connections from a variable to a net (A21-D1..D3), because a put or
  force on the sink must not reach the source (IEEE 1800-2009 38.34: a put
  to a net "overrides the resolved value of the net. This value shall
  remain in effect until one of the drivers of the net changes value"), and
  frame records while an application is loaded, because a later `vpiFrame`
  query needs every activation entered since start-up.
- **Evidence:** run time against the same model without the catalog
  (`plain`), with the catalog and no application (`vpi`), and with a no-op
  application that registers one system task (`app`); pinned medians of 9
  interleaved rounds (5 for the long designs), identical output. Besides
  the corpus and `perf/designs`, `mixed_perf`, `udp_perf`, `rec`, `calls`
  and `forks` are local stress designs (netlist stages, a UDP ring, deep
  recursion, task calls, forks):

  | design | plain ms | vpi/plain before → after | app/plain before → after | model C MB plain / vpi | C build s plain / vpi |
  | --- | ---: | --- | --- | ---: | ---: |
  | many-registers-10k | 92.4 | 1.24 → 1.02 | 1.39 → 1.03 | 2.66 / 111.75 | 0.6 / 19.2 |
  | many-masked-20k | 279.2 | 1.10 → 1.01 | 1.14 → 1.03 | 3.70 / 114.24 | 0.6 / 16.5 |
  | tasks | 81.7 | 1.01 → 1.01 | 1.04 → 1.03 | 0.28 / 4.36 | 1.7 / 2.1 |
  | zero-delay | 131.7 | 1.01 → 1.00 | 1.02 → 1.02 | 0.03 / 0.07 | 0.3 / 0.4 |
  | wide-values (400 rounds) | 63.6 | 1.00 → 1.00 | 1.01 → 1.01 | 0.06 / 0.46 | 0.5 / 0.6 |
  | mixed_perf | 689.3 | 1.92 → 1.87 | 1.91 → 1.86 | 1.15 / 17.65 | 8.8 / 11.8 |
  | udp_perf (2,000 UDPs) | 2549.2 | 0.98 → 0.98 | 0.97 → 0.95 | 3.34 / 22.91 | 11.5 / 13.5 |
  | gen64_proto | 3.4 | 1.21 → 1.13 | 1.28 → 1.14 | 0.15 / 1.95 | 0.6 / 0.7 |
  | gen64_probe | 3.4 | 1.20 → 1.12 | 1.25 → 1.26 | 0.15 / 1.95 | 0.5 / 0.7 |
  | cpu | 2.5 | 1.02 → 1.06 | 1.02 → 1.04 | 0.07 / 0.22 | 1.1 / 1.2 |
  | fifo | 2.4 | 1.03 → 1.07 | 1.07 → 1.03 | 0.04 / 0.14 | 0.5 / 0.8 |
  | uart | 2.6 | 0.99 → 0.97 | 1.05 → 1.05 | 0.04 / 0.14 | 0.5 / 0.6 |
  | comb | 2.4 | 1.04 → 1.02 | 0.97 → 1.01 | 0.04 / 0.08 | 0.5 / 0.4 |
  | rec (recursive task) | 2876.0 | 0.99 → 1.00 | 1.04 → 1.01 | 0.02 / 0.04 | 0.2 / 0.2 |
  | calls | 10591.4 | 1.01 → 1.00 | 1.04 → 1.04 | 0.02 / 0.04 | 0.2 / 0.2 |
  | forks | 347.4 | 1.01 → 1.02 | 1.04 → 1.04 | 0.03 / 0.05 | 0.2 / 0.3 |

  Model C size is the generated source; C build time is one cold build on
  a shared host. The tiny designs (`cpu` to `comb`) run in about 3 ms, where
  0.1 ms is 4%.

  Attribution, one choice at a time (generated-C toggles, relinks):
  sharing the port links again brings `mixed_perf` (500 stages of UDPs,
  gates and an `always`, clock and enable connected from variables) from
  1.88x to 1.00x, so the kept links are its whole overhead; writable gate
  delays are within noise there, and the statement counters too; without
  the counters `tasks` runs at 1.00 instead of 1.01 and `forks` at 1.01
  instead of 1.03 of plain. Sharing the links changes nothing on the designs
  without such ports (`udp_perf`, `cpu`, `fifo`, `uart`). On
  `many-registers-10k` the position-independent link applied 550,000
  relocations and kept 28 MiB more resident (+22% run time), and a
  registration indexed 60,000 catalog rows and 400,000 expression rows
  (+17 ms). The `gen64` designs keep about 0.4 ms of start-up with the
  catalog. Frame records with an application loaded cost
  up to 5% on call-heavy designs (`rec`, `calls`). The catalog dominates
  compile time on large designs (`many-registers-10k`: 0.6 s without, about
  15-19 s with it); its expression and statement rows are about 60% of that
  and the object rows about 7%. Mach-O and PE executables still relocate
  the catalog at start.

## Rejected or deferred

| Option | Effect | Decision |
| --- | --- | --- |
| Split the model into parallel translation units | Loses inlining across units. Compile: `pca-4100` 22% slower, `many-registers-10k` 8% faster, `tasks` 0.4 s saved; the serial cost was one storage function, later fixed by table-driven storage | Rejected |
| Shared prologue/epilogue helpers for all coroutines | Compile +25% on the capacity model, flat elsewhere; `tasks` simulation +3.6% (within spread) | Rejected |
| Shared resume dispatch through continuation functions | An indirect transfer per resume; compile +4% | Rejected |
| Non-inlined await-result helper | A call after every arm; compile change within noise | Rejected |
| Liveness-based frame narrowing beyond scope narrowing | Moves 9–58% of frame fields; compile flat (`continuous_contexts` 29.6 → 30.0 s) | Rejected |
| One model-wide descriptor array | GCC 12× slower (identical-code folding over the constant graph); simulation flat | Rejected |
| Bounded descriptor arrays; interning equal site tables | 18–59% smaller C; simulation flat; compile flat to +67% (`many-registers-10k`) | Deferred: size alone is not a goal. Shared body classes now have one descriptor each (see per-instance netlist data), which compiles faster |
| Inline cancellation flag instead of `llg_activation_cancelled` | At most 0.2% of samples after selective cancellation checks | Rejected |
| Computed-goto resume dispatch (GNU C) | Dispatch lines take 0–0.05% of samples; adds a compiler-specific path | Rejected |
| Split process identity from frames | Frees large finished frames early, but finished frames are 16 B each in the corpus (0.2% of peak RSS); costs an extra allocation per live process | Deferred |
| Coroutine chain record 48 → 40 B | Saves 8 B per process, no speed gain; changes every runtime service signature and the coroutine ABI | Deferred until another ABI change |
| Leaf-process fast path (`always @(sens)` bodies entered without generic dispatch, cached re-arm) | Implemented and measured: −1.1 to +0.3% on designs with leaf processes, all within noise; dispatch, marks and re-arm were each under 1% of samples. Adds a second process emission form, 8 B per waiter source row and about 1 KiB of runtime code | Rejected |
