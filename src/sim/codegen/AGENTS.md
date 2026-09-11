# Simulator lowering

Applies to `codegen.rs` and its child modules. Read [../AGENTS.md](../AGENTS.md)
for pipeline/build rules and [../rt/AGENTS.md](../rt/AGENTS.md) for runtime
contracts. `lower_expr`/`lower_stmt`/`lower_lhs` produce typed IR only.

## Lowering pitfalls

- Continuous assigns and `always_comb`/`@*` become comb processes:
  evaluate once at t=0, then `wait_any` on the RHS/body **read** set — the
  LHS base signal must NEVER be in the sensitivity list (self-wake bug).
- Event or-lists (`@(posedge a or negedge b)`) must be ONE atomic
  `llg_wait_any_events` call, never sequential waits.
- Input/output port connections become link processes, with input expressions
  evaluated in the parent's context and output selections retaining untouched
  bits. Constants and omitted-port defaults evaluate once; an explicitly open
  input does not use its default. Matching whole packed-variable `ref` ports
  share canonical storage, including nested references; they emit no copy link.
  Selected/object/array ref actuals remain unsupported. Inout ports emit no
  link — the net group IS the connection. Slang binds interface
  and modport member references directly to storage on the actual interface
  instance, where interface body processes also emit.
- `wait (cond) stmt` lowers to `for(;;){ if (sv4_to_bool(cond)) break;
  wait_any(reads(cond)); } <body>`; wait-bearing tasks are inlined.
- `$display` format strings are parsed at codegen time; `%t` consumes an
  argument (typically `$time`) — codegen and the runtime `llg_display`
  must agree on specifier/argument counts.
- Expression widths: constants, parameters, signals and concat/replication
  results are checked against the generated model's `LLG_MAX_WIDTH`; the
  backend rejects widths at its exclusive `1 << 20` limit. The IR does not
  impose a fixed 1024-bit or 64-bit arithmetic cap, and division/modulo/power
  preserve the model-sized limb width. Keep defensive checks at the backend
  and runtime boundaries; silent truncation in `sv4_concat` is a real bug.
- `for` initializer, condition, increment and body relationships come from the
  typed owned database. Do not recover them from syntax or frontend numeric
  object codes.
- Statement and intra-assignment delays retain a typed expression `NodeId`;
  continuous-assignment and primitive delays retain the ordered `DriverDelay`
  form. Evaluate these identities through owned constants or typed runtime IR,
  and round a complete real-valued delay once at the owning module precision.
  Reject separate transition delays explicitly until their scheduling is
  implemented; never use only the first expression. Unsupported forms must fail
  without recovering or guessing source text.
- Generated C uses GNU statement-expressions `({ ... })` for select-LHS
  write-back (gcc/clang OK, not strict ISO C).
- Packed streaming expressions retain Slang's resolved direction, slice size,
  ordered stream operands, aggregate width, and unsigned result;
  streaming assignment targets retain typed component LHS expressions and
  explicit component widths so the RHS is evaluated once before unpacking.
  Stream `with` selectors remain an explicit unsupported boundary.
  `inside` evaluates its selector and every scalar/range endpoint once, using
  wildcard equality for scalar items and ordinary inclusive comparisons for
  ranges.
- Dynamic arrays, queues, and associative arrays lower to distinct container
  IR and runtime storage; their packed elements retain arbitrary model width.
  The current vertical slice supports one resizable dimension, dynamic `new`
  and copy/delete, positional dynamic/queue assignment patterns, queue element/
  method operations, packed-element `sum`/`product`/`and`/`or`/`xor`
  reductions, and integral or string-key associative access, delete,
  existence, and traversal. Resizable element NBAs
  are illegal (LRM §6.21), and container reads in sensitivity/wait expressions
  are rejected until mutations can notify the scheduler. Declaration
  initializers and keyed/default container assignment patterns, resizable
  subprogram/port storage, and assignment-compatible traversal keys that
  require conversion remain explicit unsupported cases.


## Inout ports and tri-state nets

The parent and child nets of every inout port
collapse into ONE resolved simulated net (`llg_net_t`, LRM §23.3.3.7)
with one driver slot per member net.  Whole-signal writes to a member lower
to `llg_net_write`; every read (refs, `$display`/`$monitor` args,
sensitivity) goes through the shared `resolved` cell, so wire/tri
resolution (Table 6-2, equal strengths: all-Z → Z, one non-Z → that value,
equal values → that value, any X or mixed 0/1 → X) is automatic.  Groups
with non-net members, mixed widths, unsupported net types (wand/wor/
tri0/tri1/reg), select-LHS/NBA/task-actual writes are skipped with an
explicit warning; inout ports emit no link (the group IS the connection)
and input/output links touching a member are warned and skipped.

Standalone packed `wand/triand` and `wor/trior` nets use one group per
declaration and a distinct driver slot per whole-net continuous-assignment
site (including declaration assignments), in deterministic node order, up to
16 sites. Synthetic driver signals carry slot identities but no waveform names;
user reads and sensitivity observe only the shared resolved cell. All-Z/no
drivers resolve to Z; 0 dominates X for wired-AND, 1 dominates X for wired-OR.
Reject port/interface/array wired nets, hierarchical or select LHS, procedural
writes, force/release, gate and function/task-output drivers, and explicit
drive strengths. This does not change the older per-member wire/inout model.
See `tests/sim_net_resolution.rs` and standalone `tests/runtime_values.rs`.

Standalone scalar and packed `wire/tri/uwire` declarations use the same per-site
driver identity, including ordinary whole-net and selected continuous
assignments with constant indices and bounds. A selected site rebuilds its
complete contribution from Z on each evaluation before setting its
bit/part-select, so it contributes Z outside that selected range. For scalar
ordinary nets, explicit continuous-assignment drive strengths are retained per
site; §10.3.4 forbids them on vector nets. Resolution follows the Table 28-7 scale and §28.12 uncertainty ranges:
an X driver exposes both its strength0 and strength1 endpoints, so a known
driver wins only when it strictly dominates every possible opposite endpoint.
Highz endpoints contribute no drive. Dynamic net
selectors are rejected because net lvalues require constant selects; variable
lvalues remain a separate lowering path. Delayed whole-net drivers
contribute X until their first scheduled update; a truly driverless wire uses
the synthetic Z placeholder. Delayed selected drivers remain explicitly
unsupported. Port/interface nets retain the link/collapsed-inout path. A
single gate-only net and a forced net with at most one continuous driver retain
their established direct-write path; mixed gate/continuous, multiple gate, or
forced multidriver nets are rejected until those writers have independent
contribution slots. Slang rejects overlapping `uwire` drivers; disjoint
constant selected drivers remain legal and preserve Z in undriven bits.

The same bounded standalone-driver path supports `tri0/tri1` and
`supply0/supply1`. Pull defaults replace only all-Z bits after ordinary wire
resolution; X and conflicting active drivers remain X. Supply defaults dominate
ordinary implicit-strength drivers. Resolved cells start at their default before
processes run, while individual contribution slots start at Z. Explicit strengths
remain rejected for wired nets, pull/supply defaults, ports, and gates;
`trireg` charge storage is not implemented: every elaborated declaration,
including undriven nets and arrays, is rejected before collecting storage.
See `tests/sim_net_defaults.rs`.


## Initialization, generate scopes and interfaces

- True-net declaration assignments (`wire w = expr;`, including dynamic RHS)
  use the same event-driven `RunOnce`/`SensLoop` path as explicit continuous
  assignments. Reject dynamic true-net drivers reading unpacked arrays and
  unsupported resolved-net classes when sensitivity/resolution is unrepresentable.
- Ungrouped ordinary nets (including ports, interface members and net-array
  elements) start at Z. Resolved-group cells retain their resolution defaults;
  pending delayed continuous and gate-driver contributions retain their
  explicit X initialization, including collapsed inout gate drivers.
- Variable initializers run in `main()` before processes, so a t=0 process
  write wins. Order: ordinary-net defaults, unpacked-array fills, scalar `reg` fills (`reg y = 0;`),
  then scalar variable fills (`logic l = 1'b0;`, `int x = 5;`,
  `logic [7:0] v = 8'ha5;` through `Db::vars_init`).
  Fold RHS constants using collected parameters (`int y = P + 1;`); reject
  nonconstant variable initializers (`logic z = a;`).
- Slang's resolved variable lifetime controls procedural block storage.
  Static block locals use hidden model signals and initialize once before
  processes start; automatic locals remain lexical C storage initialized on
  each declaration entry. This distinction includes inherited default-static
  locals and automatic loop/block scopes; explicit qualifier syntax alone is
  not sufficient to recover it.
- Generate-scope processes inline concrete genvar parameter values. Generated
  module instances retain per-iteration paths (`top.g[0].u`), parameters,
  signals, processes and links. Array-element port actuals (`.cnt(cnts[i])`)
  require compile-time constant indices, including elaborated genvars;
  reject dynamic array-element connections.
- Interfaces support actuals, direct modport member bindings and
  parameter-folded widths. Emit interface always/initial/always_comb bodies
  under the actual instance named by the owned semantic binding.

## Control flow and procedural drivers

- Functions/tasks support recursion and defaults, including defaults referring
  to earlier formals. Inline delay/wait-bearing tasks at call sites; reject
  recursive delay-bearing tasks and task calls from function bodies.
  Per LRM 1800-2009 §§6.21 and 13.4.2 (and 1364-2001 §§10.2.3 and
  10.3.1), static subprogram formals, locals and function return variables
  retain one value per elaborated definition; inputs/inouts copy in on each
  call and outputs/inouts copy out when the task returns. Delay-bearing static
  tasks use hidden model storage because their bodies are inlined. Automatic
  subprograms retain fresh per-call storage. Constant/provenance-supported local
  declaration initializers run once for static storage and on each automatic
  call; runtime-dependent static initializers are rejected rather than evaluated
  on first call. A queued NBA may target static
  packed storage, but every NBA to an automatic formal/local is rejected
  before emission so no runtime pointer can outlive its C storage. Unpacked
  subprogram storage remains unsupported for NBA targets. Explicit local
  lifetime qualifiers matching the enclosing subprogram are accepted when
  owned capture is available; ambiguous or opposite-lifetime overrides are
  rejected because the owned DB cannot safely preserve their lifetime.
- Chandle-returning delay-free functions with chandle input formals retain
  native `void *` values through IR and C calls; static chandle inputs use
  definition-wide object storage. Output/inout chandle formals, mixed packed
  and chandle signatures, chandle locals, and delay-bearing chandle tasks are
  rejected rather than encoding pointers as integers.
- Automatic, delay-free string-returning functions with packed input formals
  return owned `llg_string_t` values. Static string returns, string/chandle
  formals, output/inout formals, and local string declarations are rejected
  until persistent string ownership and typed object-formal copy semantics are
  available.
- Fork/join works only in process bodies: join/join_any/join_none, named forks,
  `wait fork;`, `disable fork;`. Reject fork/join in function/task bodies and
  cross-process `disable <label>;`.
- Inline `for` declarations use lexical packed locals, with unique names for
  nested/shadowed declarations. `foreach` traverses fixed unpacked arrays in
  declared dimension order and requires one explicit iterator per dimension.
  Break/continue follow the innermost source loop. Reject real loop locals,
  omitted foreach iterators, fork/deferred-output captures of loop locals, and nonblocking
  writes to loop locals whose stack lifetime cannot cover NBA commit.
  `tests/sim_loops.rs` compares optimized/unoptimized execution.
- `case (...) inside` supports wildcard scalar members and inclusive ranges,
  retaining first-match/default ordering. Evaluate the selector exactly once
  into a local temporary before testing members (`tests/sim_wildcard_eq.rs`).
- `wait (expr) stmt` re-evaluates on changes to read signals, then runs its body
  once when true. Constant true executes immediately; a false/unknown constant
  remains suspended on an empty dependency set, allowing time to advance.
- Event expressions compare successive expression values, rather than waking
  for every operand change. Packed edges observe the LSB; `iff` is evaluated
  at the trigger, before resuming the waiter. Qualified named events and mixed
  event lists retain one atomic registration. Evaluator helpers use owned IR;
  array dependencies, function calls, and callbacks capturing procedural/
  subroutine locals or formals remain explicit rejections. Function callbacks
  need reentrant effect handling before they may mutate scheduler-observed
  storage while waiter lists are being traversed. See `tests/sim_partial_features.rs`.
- Whole-signal `force sig = expr;` ignores procedural blocking/NBA writes while
  forced. `release sig;` restores the pre-force value without re-evaluating
  drivers changed during force (current approximation). Re-force changes the
  forced value but preserves the original saved value. Normal signal writes
  implement force/release so `@(sig)`/wait waiters wake.
- Procedural continuous `assign <variable> = expr;` / `deassign <variable>;`
  use a pre-scanned enable-guarded process per site. Deassign disables the
  driver, retaining the last value (`tests/sim_force.rs`).

## Hierarchical references and output

Hierarchical reads (`top.u0.sig`, any resolved N-part signal path) work in
expressions/display/monitor; whole-signal blocking/NBA writes target the
resolved instance, including collapsed inout-net drivers. Slang's typed select
expressions preserve hierarchical bit, part, and indexed-part writes: bit and
indexed-part base expressions can be runtime integral values, while part-select
bounds and indexed-part widths must resolve statically. Reject an absent bound
or selector rather than recovering one from a name or source line. Single
packed dimensions use owned declared bounds to translate ascending/nonzero
ranges to storage offsets, including fixed-array element bit/part selections.
Widen index arithmetic before translation, preserving signedness and X/Z;
invalid bit indices do not write. Partially out-of-range part-select reads
retain in-range bits and fill only missing bits with X. File-based regressions
live in `tests/fixtures/sim/partial_features/*select_ranges.sv`.

`$monitor`/`$monitoron`/`$monitoroff` check changes after active/inactive/NBA settling;
only the most recent monitor is active. `$strobe` prints once with post-NBA
values for its timestep, including combinational updates triggered by NBAs.
`$write` uses display formatting without a newline.
`%d` prints two's-complement negatives when `is_signed` is set; negative
unsized decimal literals such as `-3` emit signed per the LRM.

`$dumpfile` chooses `.vcd` or `.fst`; `$dumpvars`/`$dumpon`/`$dumpoff`/
`$dumpall`/`$dumpflush`/`$dumplimit` lower through IR to asynchronous waveforms.
Dumpvars depth/scope arguments warn then select all registered user storage.
Models without waveform controls omit the waveform runtime and GTKWave libfst
sources. Reject extended-VCD `$dumpports`; `$displayon`/`$displayoff` warn and
skip. Basic SystemVerilog `string` storage is lowered for bounded module and
generate-scope paths, and automatic packed-input functions may return string
values. String ports, general string subprogram storage/formals,
and continuous-assignment/sensitivity paths remain unsupported. `atoreal`
returns a real-valued expression from a decimal prefix, and `realtoa` applies
ordinary real argument conversion before replacing the string value.

Packed Verilog string literals are unsigned integral byte vectors, with the
leftmost character most significant. Slang supplies owned decoded bytes before
the generated model-width limit is checked; an empty literal is one zero byte.
Assignments pad/truncate as packed values, and explicitly packed parameters
can initialize packed storage. SystemVerilog `string`-typed storage is
separate from packed Verilog byte vectors: basic declarations, assignment/
copy, casts, display paths, and bounded automatic function returns are covered,
while general string subprogram forms, ports, and continuous-assignment/sensitivity
paths remain unsupported. See the bounded
`tests/fixtures/sim/data_types_next/readme.md` inventory.

## Values and real numbers

Keep `LLG_MAX_WIDTH` aligned with the generated model definition in
`llg_value.h`, including constant, parameter, signal, concat and replication
checks. The exclusive backend capacity is `1 << 20`; div/mod/pow are
model-width operations, not a separate 64-bit subset. The IR remains
backend-independent and does not repeat this capacity as a semantic limit.
X/Z remain distinct for display, literal equality and casez/casex matching
(LRM 12.5.1); Z behaves as X in other expression contexts (LRM 11.4.5), with
identity/copy operations preserving it. See runtime value contracts.

Unbased unsized fills (`'0/'1/'x/'z`) expand in context-determined packed
operands, including arithmetic/bitwise expressions, comparisons, conditional
branches, assignments, and function arguments. Ordinary case/casez/casex
selectors and items share the maximum operand width and common signedness.
Concatenation/replication operands and other self-determined positions stay
one bit. The semantic importer must preserve the fill operation explicitly;
never infer it from an unrelated source token. Context widening must preserve model-sized division,
modulo, and power operations. See `tests/sim_fill_literals.rs`.

Wildcard equality (`==?`/`!=?`) treats only RHS X/Z bits as wildcards after
common-width and signedness conversion. A known mismatch wins over an unknown
LHS bit at another position; otherwise an unmasked LHS X/Z yields X. Real
operands are rejected. `tests/sim_wildcard_eq.rs` compares optimizer variants.

`$countones`, `$onehot`, `$onehot0`, and `$isunknown` accept one packed
integral expression up to the generated model's `LLG_MAX_WIDTH`. One counts ignore X/Z positions;
`$isunknown` detects either state. Count results are signed 32-bit integers,
and predicates are unsigned one-bit values. The argument evaluates once;
optimizer read collection and combinational sensitivity retain its dependencies.
Real operands are rejected. See `tests/sim_bit_queries.rs`.

Procedural scalar `real`/`shortreal` use companion `double` storage. B6 supports
constant initialization, real parameters, blocking/NBA assignment, mixed
packed/real arithmetic, relational/logical operations, conditionals, casts,
`if`/`while`/`for` conditions and display `%f`/`%e`/`%g` width/precision.
Shortreal assignment rounds through C `float`; real-to-packed rounds nearest
(halves away from zero) for targets up to the generated model width.
Packed-to-real accepts the model width, treating X/Z positions as zero.

Slang supplies typed explicit conversions and resolved real parameters.
Lowering consumes those operations directly and must not reconstruct
comparisons by parsing source text.

`$rtoi` truncates toward zero into signed 32-bit storage (non-finite inputs
yield X; finite overflow wraps modulo 2^32). `$itor` preserves integral
arguments' packed width/signedness; a real argument first undergoes ordinary
rounded conversion to a signed 32-bit integer. Functions taking real values
accept implicit packed-to-real coercion. `$realtobits`/`$bitstoreal` and
`$shortrealtobits`/`$bitstoshortreal` reinterpret IEEE-754 representations;
the inverse bitcasts require exactly 64/32 bits, with X/Z positions treated
as zero. Typed parameters and constant declaration initializers use the same conversion rules.
See `tests/sim_real_conversions.rs`.

All 21 real mathematical functions from IEEE 1800-2009 table 20-4 lower to
validated typed IR and the specified C math functions. Each argument evaluates
once with numeric real conversion; domain/non-finite behavior follows C libm.
`$realtime` returns fractional module-unit time without integral rounding.
These procedural real expressions retain the real-context restrictions below.
See `tests/sim_partial_features/system_functions.rs`.

Reject real ports/links, arrays, function/task types, continuous/comb processes,
real event controls/wait conditions, monitor/strobe args, force/release/select/
case/repeat contexts, and bitwise/reduction/shift/concat/case-equality operations
before C compilation. `tests/sim_real.rs` pins support and rejection messages.

## Timescale

- Continuous/gate delay lowering uses `IrStmt::InertialAssign` for whole packed
  driver storage. Its evaluation never suspends: the runtime captures the
  converted value and maintains one cancelable active-region propagation event
  per site. Delayed driver initialization is emitted after ordinary storage
  defaults, including output ports and collapsed net slots. Keep driver values
  separate from the net's resolved value when comparing pending updates.

- Delays are timescale-aware: codegen reads the resolved time unit and
  precision from the nearest owning Slang module instance and scales every
  delay by `N * unit / design_precision` before calling `llg_wait_time`.
  `$time` returns the current time in the calling module's unit
  (`llg_time() * design_precision / unit`), so `%t`/`%0d` displays show the
  unit-scaled time; `$printtimescale` prints the calling module's
  unit/precision. Compilation-unit and declaration inheritance are frontend
  responsibilities.
- The scheduler runs in design-precision ticks: the design precision is the
  FINEST precision across every module (default 1ns/1ps for modules without a
  directive), so 1 tick = design_precision ps.  The runtime itself stays
  timescale-agnostic (`llg_wait_time` receives already-scaled ticks), so no
  runtime change was needed.
- `core::db` retains the typed expression identity for statement and
  intra-assignment delays. Lowering evaluates resolved integer/real parameters,
  casts, integer operations, and real/time arithmetic without reading source
  spelling. It rounds the complete real-valued delay once to the local precision,
  then converts to nonnegative 64-bit scheduler ticks. Ordinary time-literal
  value expressions preserve Slang v11's unrounded, module-scaled `real` value;
  parameter and declaration initializers use the same typed constant path.
  Runtime procedural expressions remain typed `IrDelay` operands with the
  owning module's unit/precision scaling. Evaluate them once when encountered;
  X/Z means zero delay, and negative packed values convert to unsigned 64-bit
  time before checked scaling. Runtime real values round once to local
  precision; nonfinite/negative real values and tick overflow fail explicitly.
  Sub-picosecond precision still clamps up to 1 ps in the runtime representation.
  See `tests/sim_delay.rs`, `tests/sim_time_literals.rs`, and `tests/sim_time_values.rs`.

## Unpacked arrays and memories

- Storage: every array becomes a flat C array `sv4_t G_<path>_<name>[N]`
  (`N` = product of the per-dimension sizes `|left - right| + 1`); elements
  start all-X for four-state variable elements, zero for two-state elements,
  or Z for ordinary net elements (a loop
  in `main()` fills them, since a function call is not a
  valid static initializer).  Declaration initializers (`= '{…}`) — captured
  by `core::db` on the declaration initializer relationship — are applied in `main()` before any
  process runs; each pattern operand is a constant, in linear-index order.
- Indexed access: `mem[i]` (1-D), `a[i][j]` (N-D) and element-level selects
  `mem[i][3:0]` / `mem[i][2]` / `mem[i][base +: width]` lower on both read
  and write paths. Indexed part-selects use a constant width and normalize the
  runtime base/direction against the declared packed range.
  The linear index is row-major with the **leftmost dimension slowest**
  (matching Verilog); descending ranges (`[255:0]`) map `left` to offset 0.
- Out-of-range semantics (matching Verilog): an index outside the declared
  bounds, or with unknown (X/Z) bits, reads as the element type's default
  value (X for four-state, zero for two-state) and makes a write a no-op.
  Guard code uses `sv4_to_index_i64` to preserve index signedness and reject
  high-limb overflow, checks declared bounds before subtracting offsets, then
  computes the flat element address.
- Rejected with a clear message: dimension bounds that are not resolved
  constants, array slices
  (`a[i]` on a 2-D array — partial indexing), non-constant declaration-initializer elements,
  and arrays wider than `LLG_MAX_WIDTH` per element.
- Nonblocking writes capture the element address and RHS when issued. Bit/part/indexed-part
  writes store an update mask and merge into current storage at NBA commit,
  preserving disjoint updates and intervening writes to other bits. The same
  contract applies to packed selections and constant/runtime-delay NBAs. Future NBAs
  outlive their issuing process and do not suspend it; blocking delayed
  assignments retain the capture-then-suspend path, including real/shortreal
  values captured in local C doubles before assignment conversion.
- Comb-sensitivity limitation: an `always_comb`/`@*` process reading an array
  element wakes only on its index signals, not on writes to the array
  (element writes through `llg_ba`/`llg_nba` still notify waiters watching
  that exact element address, but the generated comb processes do not watch
  array elements).  Use event-controlled (`always_ff`/`@(...)`) processes for
  memory reads.

## Structures and unions

Packed untagged structures and unions lower as packed values. Packed-union
members overlay bit zero, must have equal resolved widths, and retain each
member's signedness and 2-state conversion on named access. Tagged unions are
rejected.

The unpacked aggregate vertical slice covers top-level module and generate-scope
variables whose members are fixed-width packed integral values. Unpacked struct
members have independent typed signal storage; equal-width unpacked untagged
union members share storage. Named member reads/writes and constant member
bit/part selects are supported. A whole assignment between compatible named
unpacked types is fieldwise for structs and copies shared storage for unions.
Declaration and procedural assignment patterns support positional, complete
member-named, default, and simple integral type keys. Type keys match exact
packed dimensions, signedness, and 2-state/4-state domain; member keys take
precedence over type keys, and the last matching type key takes precedence over
default. Explicit nested packed-aggregate member patterns are recursive.
Recursive default/type-key distribution into aggregate-valued members and
nominal aggregate type keys fail closed until owned lexical type identity is
available.
Anonymous whole-type copies fail closed when owned type identity is unavailable.
Reject unpacked aggregate nets/ports, nested aggregate or unpacked-array members,
unequal-width unpacked unions, tagged unions, aggregate subprogram
formals/locals, compound assignments, and whole aggregates in scalar expression
contexts.
