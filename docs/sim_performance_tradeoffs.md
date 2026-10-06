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

### Timing tasks as calls, not inline expansions

- **Trade-off:** inlining saves a call and frame per invocation; calls avoid one
  copy of the task body per call site.
- **Decision:** event, disable and timed tasks are called. Tasks with `ref`
  event formals are specialized per static actual instead of inlined.
- **Evidence:** on `testbench_tasks` N=128, `model.c` shrinks from 20.2 to
  5.8 MB and its compile from 108 to 27 s. `tasks` simulation drops from 2.48
  to 1.60 s, because the smaller code fits caches better. (That A/B also
  includes compact identifiers, which do not affect run time.)

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
| Runs of same-shape procedural continuous `assign`s | Row table and one helper per shape, from `PCA_BATCH_MIN_ASSIGNMENTS` (4) rows | Capacity model build 14.8 → 2.3 s | 135.6 → 136.7 ms (noise) |

Both thresholds are 4 because shorter runs gained at most a few percent of
compile time. Below the threshold, emission is unchanged.

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

## Rejected or deferred

| Option | Effect | Decision |
| --- | --- | --- |
| Split the model into parallel translation units | Loses inlining across units. Compile: `pca-4100` 22% slower, `many-registers-10k` 8% faster, `tasks` 0.4 s saved; the serial cost was one storage function, later fixed by table-driven storage | Rejected |
| Shared prologue/epilogue helpers for all coroutines | Compile +25% on the capacity model, flat elsewhere; `tasks` simulation +3.6% (within spread) | Rejected |
| Shared resume dispatch through continuation functions | An indirect transfer per resume; compile +4% | Rejected |
| Non-inlined await-result helper | A call after every arm; compile change within noise | Rejected |
| Liveness-based frame narrowing beyond scope narrowing | Moves 9–58% of frame fields; compile flat (`continuous_contexts` 29.6 → 30.0 s) | Rejected |
| One model-wide descriptor array | GCC 12× slower (identical-code folding over the constant graph); simulation flat | Rejected |
| Bounded descriptor arrays; interning equal site tables | 18–59% smaller C; simulation flat; compile flat to +67% (`many-registers-10k`) | Deferred: size alone is not a goal |
| Inline cancellation flag instead of `llg_activation_cancelled` | At most 0.2% of samples after selective cancellation checks | Rejected |
| Computed-goto resume dispatch (GNU C) | Dispatch lines take 0–0.05% of samples; adds a compiler-specific path | Rejected |
| Split process identity from frames | Frees large finished frames early, but finished frames are 16 B each in the corpus (0.2% of peak RSS); costs an extra allocation per live process | Deferred |
| Coroutine chain record 48 → 40 B | Saves 8 B per process, no speed gain; changes every runtime service signature and the coroutine ABI | Deferred until another ABI change |
| Leaf-process fast path (`always @(sens)` bodies entered without generic dispatch, cached re-arm) | Implemented and measured: −1.1 to +0.3% on designs with leaf processes, all within noise; dispatch, marks and re-arm were each under 1% of samples. Adds a second process emission form, 8 B per waiter source row and about 1 KiB of runtime code | Rejected |
