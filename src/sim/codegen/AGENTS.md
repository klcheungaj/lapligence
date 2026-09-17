# Simulator lowering

Applies to `codegen.rs` and its children. Read [../AGENTS.md](../AGENTS.md) for
pipeline/build rules and [../rt/AGENTS.md](../rt/AGENTS.md) for runtime contracts.
`lower_expr`/`lower_stmt`/`lower_lhs` produce typed IR only. Responsibility-named
modules live under `lowering/`; shared state stays in `lowering.rs`. Preserve
the smallest existing visibility boundary. See the
[source map](../../../docs/source_layout.md).

## Dynamic ownership and emitter boundary

Packed values use exact-width runtime storage. `LLG_MAX_WIDTH` is the Rust-side
upper bound (`LLG_SUPPORTED_WIDTH_LIMIT - 1`), not a per-model allocation
capacity; div/mod/pow and conversions use the operand or result width they need.
The structured owned emitter is the active C11 path and keeps feature guards when
it cannot establish setup, ownership, or cleanup. A few legacy lowerer paths still
produce `Verbatim` IR for the fenced fragment emitter; do not add new detached C
fragments or extend those paths as a workaround.

## Processes, expressions and delays

- Continuous assigns and `always_comb`/`always_latch`/`@*` evaluate at t=0, then
  `wait_any` on RHS/body **reads**, never the LHS base (self-wake bug).
  Ordinary `always` remains a repeated procedural loop, even without timing.
  Generated back-edges enforce cooperative zero-time budgets and report the
  owning source location; do not replace them with comb/run-once nodes.
- Event or-lists (`@(posedge a or negedge b)`) require one atomic
  `llg_wait_any_events`, never sequential waits. `wait (cond) stmt` uses packed
  `sv4_to_bool` or scalar real truth and re-evaluates on typed storage changes.
  Inline wait-bearing tasks. For `wait (expr) stmt`, true executes the body
  once immediately; false/unknown constants suspend on an empty dependency
  set without preventing time advancement.
- Input/output links evaluate inputs in the parent context and preserve
  untouched output-selection bits. Constants and omitted-port defaults run
  once; explicitly open inputs ignore defaults. Matching whole packed-variable
  `ref` ports, including nested references, share canonical storage without
  copy links. Scalar `real`/`shortreal` links use doubles and notify changed
  real dependents. Reject selected/object/array ref actuals. Inouts use their
  net group, not a link. Interface/modport references and body processes use
  storage on the actual interface instance bound by Slang.
- Parse `$display` formats during lowering. `%t` accepts integral/real values
  (usually `$time`/`$realtime`) with their owning physical unit; `$timeformat`
  arguments remain runtime expressions updating design-wide state.
- Check constants, parameters, signals, concat and replication against the
  backend `LLG_MAX_WIDTH` (`LLG_SUPPORTED_WIDTH_LIMIT - 1`), aligned with
  generated `llg_value.h`. The exclusive backend limit is `1 << 20`; IR has no
  fixed 1024-/64-bit arithmetic cap. Keep backend/runtime checks; never silently
  truncate `sv4_concat`.
- Get `for` initializer/condition/increment/body relationships from typed owned
  data, not syntax or frontend numeric codes. Statement/intra-assignment
  delays retain expression `NodeId`; continuous/primitive delays retain ordered
  `DriverDelay`. Evaluate owned constants or typed runtime IR and round a
  complete real delay once at module precision. Preserve single, rise/fall,
  and rise/fall/turn-off forms; select delay per changed bit, using the minimum
  applicable delay for ambiguous X transitions. Reject unsupported forms
  without guessing source text or keeping only the first expression.
- `->` triggers in Active. `->>` queues an NBA event: capture delay timing at
  issue; event/repeat timing uses an independent `join_none` waiter whose final
  trigger remains an NBA. Detached repeat counts must be constant until
  activation capture is represented.
- Select-LHS targets retain typed indices, ranges and write-back metadata in IR.
  The old fragment emitter still has GNU statement-expression code for some
  legacy paths, but the structured owned emitter rejects those fragments and
  emits standard C11 setup/calls/cleanup. Packed streams retain resolved
  direction, slice size, ordered operands, aggregate width and unsigned result.
  Streaming targets retain typed component LHSs/widths and evaluate RHS once
  before unpacking.
  Stream `with` selectors remain unsupported. `inside` evaluates selector and
  every scalar/range endpoint once; scalars use wildcard equality, ranges
  ordinary inclusive comparison.
- Dynamic arrays, queues and associative arrays have distinct IR/storage and
  exact-width packed elements bounded by the backend limit. The bounded slice covers one resizable dimension,
  dynamic `new`/copy/delete, positional dynamic/queue patterns, queue elements
  and methods, `sum`/`product`/`and`/`or`/`xor` reductions, and integral/string-key
  associative access/delete/existence/traversal. Reject resizable-element NBAs
  (LRM §6.21), sensitivity/wait reads until mutations notify the scheduler,
  declaration initializers, keyed/default container patterns, resizable
  subprogram/port storage, and traversal keys requiring assignment conversion.

## Inout ports and resolved nets

Collapse each inout's parent/child nets into one `llg_net_t` (LRM §23.3.3.7),
with one driver slot per member. Whole writes use `llg_net_write`; refs,
`$display`/`$monitor`, sensitivity and other reads use `resolved`. Apply Table
6-2 and Table 28-7 strength endpoints: all-Z → Z, one non-Z → its value,
equal-strength conflicts → X. Warn and skip groups with non-net members,
mixed widths, unsupported wand/wor/tri0/tri1/reg types, or select-LHS/NBA/task-
actual writes. Inouts emit no links; warn and skip input/output links touching
members.

Standalone packed `wand/triand` and `wor/trior` use one group per declaration,
with up to 16 whole-net continuous-assignment sites (including declaration
assignments), ordered deterministically by node. Synthetic slots have no
waveform names; reads/sensitivity observe only the resolved cell. All-Z/no
sources → Z; wired-AND 0 dominates X, wired-OR 1 dominates X. Retain scalar
continuous, gate and port strengths. Reject port/interface/array wired nets,
hierarchical/selected LHSs, procedural writes, force/release and function/task-
output drivers. Keep the older per-member wire/inout model unchanged. Tests:
`tests/sim_net_resolution.rs`, standalone `tests/runtime_values.rs`.

Standalone scalar/packed `wire/tri/uwire` uses the same per-site identity for
whole and constant-selected continuous assignments. Rebuild each selected
contribution from Z on every evaluation; undriven bits remain Z. Explicit
continuous strengths are scalar-only (§10.3.4 forbids vectors). Table 28-7 and
§28.12 apply: X exposes both strength0/strength1 endpoints; a known driver wins
only when strictly stronger than every possible opposite endpoint; highz adds
no drive. Reject dynamic net selectors; variable lvalues have a separate path.
Delayed whole drivers start X until their first update; genuinely driverless
wires use synthetic Z. Delayed packed selections are masked per site; fixed-
unpacked array selections have one inertial handle per element, so index
changes cancel only that element's event. Port/interface nets retain links or
collapsed inouts. Each gate output has an independent canonical slot, including
mixed gate/continuous and multiple-gate nets. Standalone net force overlays the
resolved cell while live strength-bearing slots continue updating; release
recomputes selected/multidriver targets. Slang rejects overlapping `uwire`
drivers; disjoint constant selections remain legal with Z elsewhere.

The same bounded standalone path supports `tri0/tri1` pull and
`supply0/supply1` supply defaults as implicit strength-bearing sources.
Equal-strength opposition may yield X; stronger drives override defaults.
Resolved cells start at their default, contribution slots at Z. Retain scalar
strengths for wired/pull/supply nets, ports and gates; reject explicit vector
continuous strengths (§10.3.4). Reject every `trireg` declaration before
storage collection, including undriven nets and arrays: no charge storage.
See `tests/sim_net_defaults.rs`.

## Initialization, generate scopes and interfaces

- True-net declaration assignments (`wire w = expr;`, including dynamic RHS)
  use explicit continuous assignments' `RunOnce`/`SensLoop` path. Reject dynamic
  true-net drivers reading unpacked arrays or requiring unrepresentable
  sensitivity/resolution. Ungrouped ordinary nets, ports, interface members
  and net-array elements start Z. Preserve resolved-group defaults and pending
  delayed continuous/gate X contributions, including collapsed inout gates.
- Initialize variables in `main()` before processes, allowing a t=0 process
  write to win. Order: ordinary-net defaults, unpacked-array fills, scalar
  `reg` fills (`reg y = 0;`), scalar variables (`logic l = 1'b0;`,
  `int x = 5;`, `logic [7:0] v = 8'ha5;` through `Db::vars_init`). Fold collected
  parameters (`int y = P + 1;`). Runtime-dependent scalar initializers
  (`logic z = a;`) use owned declaration identity and edition-specific phase
  for supported packed type/lifetime combinations. Reject recursive aggregates
  and unsupported subprogram storage.
- Use Slang's resolved lifetime, not explicit qualifier spelling alone. Static
  block locals use hidden model signals initialized once before processes;
  automatic locals use lexical C storage initialized per entry. Include
  inherited default-static locals and automatic loop/block scopes.
- Inline concrete genvars; preserve per-iteration instance paths (`top.g[0].u`),
  parameters, signals, processes and links. Array-element actuals
  (`.cnt(cnts[i])`) require constant indices, including elaborated genvars;
  reject dynamic connections. Interfaces support actuals, direct modport
  bindings and parameter-folded widths; emit always/initial/always_comb bodies
  under the bound actual instance.

## Subprograms, control flow and procedural drivers

- Functions/tasks support recursion and defaults referencing earlier formals.
  Inline delay/wait-bearing tasks; reject recursive delay-bearing tasks and
  task calls from functions. Per 1800-2009 §§6.21/13.4.2 and 1364-2001
  §§10.2.3/10.3.1, static formals/locals/returns retain definition-wide storage;
  input/inout copy-in occurs per call and output/inout copy-out at return.
  Inlined timed static tasks use hidden model storage; automatic routines get
  fresh per-call storage. Constant/provenance-supported local initializers run
  once for static storage, per automatic call otherwise. Reject runtime-dependent
  static initializers rather than evaluating on first call. NBAs may target
  static packed storage, never automatic formals/locals or unpacked subprogram
  storage. Resolve each explicit local qualifier: static-in-automatic persists,
  automatic-in-static is per-call. Reject unavailable/ambiguous provenance;
  never infer it from spelling or the enclosing routine.
- Admitted chandle-returning functions and chandle inputs/locals/fields retain
  `void *` through IR/C calls; static inputs use definition-wide object storage.
  Output/inout/ref/const-ref aliases and bounded delayed tasks use typed native
  ownership. Reject ports, packed containment, arithmetic, continuous/sensitivity
  paths and unsupported timed/native captures; never encode pointers as integers.
- Admitted string expressions, locals, returns, string/chandle addresses and
  scalar string formals use typed native ownership in the structured emitter.
  Automatic string NBA destinations, unsupported aggregate/continuous paths and
  arbitrary native/shared captures remain rejected. Automatic string-key
  `foreach` iterators are the bounded loop-scoped exception.
- DPI-C imports use canonical `svdpi.h` thunks for scalar `bit`/`logic`/`reg`,
  two-state integral atoms, real/shortreal, chandle and string formals. Preserve
  owned C names and pure/context qualifiers. Missing/conflicting explicit
  libraries or signatures fail before simulation; packed/open arrays, `ref`,
  exports and context callbacks remain deferred.
- Fork/join supports process bodies and legal detached `join_none` branches in
  automatic packed subroutines. Owned activation frames release captures on
  completion/cancellation. Reject blocking function joins, recursive timed
  tasks, richer subroutine storage and cross-process `disable <label>;`.
  The bounded process API supports `process::self()`, status/equality,
  kill/suspend/resume/await and automatic/static handles; reject process
  formals, arrays and broader class APIs.
- Inline `for` locals are lexical packed/real storage with unique nested/shadow
  names. `foreach` follows fixed-array declared dimension order, preserves
  omitted dimensions, and supports dynamic arrays, queues and integral/string
  associative keys. Break/continue targets the innermost source loop. Reject
  NBAs outliving loop-local storage, nested resizable elements and string-
  iterator captures. Packed/real captures retain each iteration's value in
  typed owned frames. `tests/sim_loops.rs` compares optimizer modes.
- `case (...) inside` evaluates its selector once into a temporary, preserves
  first-match/default ordering, and accepts wildcard scalar items/inclusive
  ranges (`tests/sim_wildcard_eq.rs`).
- Event expressions compare successive values, not every operand change.
  Packed edges use the LSB; real controls are any-change IEEE-bit comparisons
  (signed zero wakes, identical NaN payloads do not). Evaluate `iff` at trigger
  before resuming. Qualified named/mixed events register atomically. Evaluators
  use owned IR; bounded automatic numeric expression-only/const-ref calls with
  no side effects may be evaluated in owned frames, while array dependencies,
  mutating or otherwise effectful callbacks and unsupported captures remain
  rejected. See
  `tests/sim_partial_features.rs`.
- Force uses live evaluators with packed/real dependencies; procedural writes
  remain beneath it and net slots keep updating. Release retains a variable's
  forced value, resumes active PCA, or resolves current net drivers. Preserve
  constant-selected net parts and packed-concat descriptors; reject variable
  selects, arrays and automatic/local references. Re-force replaces the live
  matching entry. Normal signal writes must wake `@(sig)`/wait observers.
- Procedural `assign <variable> = expr;` / `deassign <variable>;` uses one
  pre-scanned enable-guarded process per site. Deassign retains the last value
  (`tests/sim_force.rs`). Reject subroutine assignments and RHS captures until
  the guard owns an activation environment.

## Hierarchical references and output

Resolved N-part paths (`top.u0.sig`) support expression/display/monitor reads
and whole blocking/NBA writes, including collapsed inout drivers. Preserve
Slang's typed hierarchical bit/part/indexed-part selections. Bit/indexed bases
may be runtime integral values; part bounds and indexed widths must be static.
Reject absent bounds/selectors rather than recovering names/source lines.
Translate ascending/nonzero single packed dimensions and fixed-array element
selects using owned bounds. Widen signed/X/Z-aware index arithmetic first;
invalid bit writes do nothing. Partial out-of-range reads preserve valid bits,
filling only missing bits with X. Regressions:
`tests/fixtures/sim/partial_features/*select_ranges.sv`.

`$monitor`/`$monitoron`/`$monitoroff` report after Active/Inactive/NBA settling.
Registration/enabling forces one report; only signal-valued arguments trigger
later reports, and only the latest monitor is active. Reject deferred output
from subprograms/captures until callbacks own their environment. `$strobe`
prints once per timestep using post-NBA values, including NBA-triggered comb
updates. `$write` omits the newline. `%d` respects two's-complement
`is_signed`; unsized decimal negatives such as `-3` are signed.

`$dumpfile` selects `.vcd`/`.fst`; `$dumpvars`/`$dumpon`/`$dumpoff`/`$dumpall`/
`$dumpflush`/`$dumplimit` lower through IR to asynchronous output. Preserve owned
`$dumpvars` depth/source identities and match the catalog before the fixed
header. Omit waveform runtime/libfst from models without controls. Reject
`$dumpports`; warn/skip `$displayon`/`$displayoff`. Bounded module/generate
SystemVerilog `string` declarations, assignment/copy, casts, display,
formals/locals/returns and automatic packed-input returns are supported.
Automatic string NBA destinations, general aggregate/continuous/sensitivity
paths and unsupported native captures remain rejected. `atoreal` parses a
decimal prefix; `realtoa` converts its real argument before replacement.
Packed Verilog literals remain separate unsigned byte vectors (leftmost byte
most significant). Decode owned bytes before width checks; empty is one zero
byte. Assignments pad/truncate; explicitly packed parameters may initialize
packed storage. See `tests/fixtures/sim/data_types_next/readme.md`.

## Values and real numbers

Preserve X/Z distinction for display, literal equality, casez/casex (LRM
12.5.1); Z behaves as X elsewhere (LRM 11.4.5), except identity/copy preserves
it. Keep the backend supported-width rules above and runtime value contracts.

Unbased fills (`'0/'1/'x/'z`) expand in context-determined arithmetic/bitwise,
comparison, conditional, assignment and argument operands. Ordinary
case/casez/casex uses maximum operand width and common signedness. Concat,
replication and other self-determined positions remain one bit. Preserve the
imported fill operation; never infer it from unrelated tokens. Context widening
must retain wide div/mod/pow (`tests/sim_fill_literals.rs`).

`==?`/`!=?` makes only RHS X/Z wildcard after common width/sign conversion.
A known mismatch beats an unknown elsewhere; otherwise unmasked LHS X/Z → X.
Reject reals. `$countones`, `$onehot`, `$onehot0`, `$isunknown` evaluate one
exact-width packed argument once and retain optimizer/sensitivity dependencies.
One-counts ignore X/Z; unknown query detects either. Counts are signed 32-bit,
predicates unsigned one-bit; reject real arguments. Tests:
`tests/sim_wildcard_eq.rs`, `tests/sim_bit_queries.rs`.

B6 scalar `real`/`shortreal` uses companion `double` storage: constant init,
real parameters, blocking/NBA, scalar ports, mixed arithmetic, relational/logical
operations, ordinary real `case`, conditionals/casts, `if`/`while`/`for`/`wait`,
comb sensitivity, any-change events, and `%f`/`%e`/`%g` width/precision.
Shortreal assignments round through C `float`; real-to-packed rounds nearest,
halves away from zero, up to the exclusive supported-width limit. Packed-to-real
accepts every legal runtime width with X/Z positions zero. IEEE-bit event comparison observes signed-zero and
changed NaN payloads, not identical NaNs. Consume typed conversions/resolved
real parameters directly; never reconstruct comparisons from source text.

`$rtoi` truncates to signed 32-bit (nonfinite → X; finite overflow modulo
2^32). `$itor` preserves packed width/sign; real arguments first round to
signed 32-bit. Real-valued functions accept implicit packed-to-real coercion.
`$realtobits`/`$bitstoreal` and `$shortrealtobits`/`$bitstoshortreal` reinterpret
IEEE representations; inverse casts require exactly 64/32 bits, X/Z → zero.
Typed parameters/constant initializers share these rules
(`tests/sim_real_conversions.rs`). All 21 IEEE 1800-2009 Table 20-4 math
functions use validated IR/C math, one numeric-real evaluation per argument,
and C libm domain/nonfinite behavior. `$realtime` retains fractional module
units (`tests/sim_partial_features/system_functions.rs`).

Retain the real-context rejection boundary: arrays, function/task types,
continuous assignments, monitor/strobe arguments, variable selects, force/release
selects, `casez`/`casex`, `inside` selectors, repeat, bitwise/reduction/shift/
concat/case-equality operations. Reject before C compilation;
`tests/sim_real.rs` pins support and diagnostics.

## Timescale

- `IrStmt::InertialAssign` supports whole/selected packed and fixed-unpacked
  drivers without suspending. Capture converted values in one cancelable
  Active event per driver (per array element). Initialize delayed contributions
  after defaults, including output/inout slots. Compare pending driver values,
  not resolved nets. Choose transition-specific single/rise-fall/three-way
  delays when scheduling; ambiguous X uses the minimum applicable endpoint.
- Read unit/precision from the nearest owning Slang module. Scale delays by
  `N * unit / design_precision` before `llg_wait_time`. `$time`/`$stime` use
  `llg_time_scaled`, rounding to calling-module units with exact halves upward;
  `$realtime` keeps fractions. `%t` converts that owning unit through design-wide
  `$timeformat` units/precision/suffix/minimum width. `$printtimescale` prints
  the caller's unit/precision; compilation-unit/declaration inheritance belongs
  to the frontend.
- Scheduler ticks use the finest design precision across modules (default
  1ns/1ps without directives): 1 tick = design_precision ps. Runtime remains
  timescale-agnostic and receives scaled ticks. Sub-picosecond precision still
  clamps to 1 ps in this representation.
- `core::db` preserves delay identity. Evaluate typed integer/real parameters,
  casts, integer operations and real/time arithmetic without source spelling;
  round the complete real delay locally before nonnegative 64-bit ticks.
  Ordinary time literals keep Slang v11's unrounded module-scaled `real` value;
  parameters/initializers share that constant path. Runtime `IrDelay` evaluates
  once with owning unit/precision: packed X/Z → zero; negative packed values
  convert to unsigned 64-bit time before checked scaling. Real delays round
  once; reject nonfinite/negative real values and tick overflow. Tests:
  `tests/sim_delay.rs`, `tests/sim_time_literals.rs`, `tests/sim_time_values.rs`.

## Unpacked arrays and memories

- Flatten to `sv4_t G_<path>_<name>[N]`, where `N` multiplies each dimension's
  `|left - right| + 1`. `main()` loops fill four-state variables with X,
  two-state with zero, ordinary nets with Z (calls are invalid static
  initializers). Apply declaration `= '{…}` relationships captured by
  `core::db` before processes, with constant operands in linear-index order.
- Support reads/writes of `mem[i]`, `a[i][j]`, `mem[i][3:0]`, `mem[i][2]`,
  and `mem[i][base +: width]`. Indexed widths are constant; normalize runtime
  base/direction against packed bounds. Row-major order makes the **leftmost
  dimension slowest**; `[255:0]` maps `left` to offset zero.
- Out-of-range/X/Z indices read the element default (X or two-state zero) and
  do not write. `sv4_to_index_i64` retains sign, rejects high-limb overflow,
  checks bounds before offset subtraction, then computes flat addresses.
  Reject unresolved bounds, partial slices (`a[i]` on 2-D), nonconstant
  initializer elements, and element widths at or beyond the exclusive backend
  limit (`LLG_MAX_WIDTH + 1`).
- NBAs capture address/RHS at issue. Bit/part/indexed masks merge into current
  storage at commit, preserving disjoint/intervening writes; packed selections
  and constant/runtime-delay NBAs share this rule. Future NBAs outlive the
  issuer without suspension. Blocking delayed assignment captures then
  suspends; real/shortreal values use local doubles before assignment conversion.
- Retain the comb-array limitation: `always_comb`/`@*` reads wake on index
  signals, not array writes. `llg_ba`/`llg_nba` notifies exact-element waiters,
  but generated comb processes do not watch elements. Use `always_ff`/`@(...)`
  for memory reads.

## Structures and unions

Packed untagged structs/unions are packed values. Union members overlay bit
zero, require equal widths, and preserve named member signedness/two-state
conversion. Reject tagged unions.

The unpacked slice supports module/generate variables with fixed-width packed
integral members: structs use independent typed signals, equal-width untagged
unions share storage. Support named reads/writes, constant member bit/part
selects, fieldwise compatible named struct copies and shared-storage union
copies. Declaration/procedural patterns support positional, complete named,
default and simple integral type keys. Match exact packed dimensions, sign and
state domain; member keys beat type keys, last matching type key beats default.
Explicit nested packed-member patterns recurse. Reject recursive default/type-
key distribution and nominal aggregate keys until owned lexical type identity
exists; reject anonymous copies without identity. Also reject aggregate nets/
ports, nested aggregate/unpacked-array members, unequal unpacked unions,
aggregate subprogram formals/locals, compound assignment and whole aggregates
in scalar expressions.
