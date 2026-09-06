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
- Port connections become link processes (input: child←parent; output:
  parent←child) — no aliasing, so edge detection stays per-signal.  Inout
  ports emit no link — the net group IS the connection.  Interface body
  processes emit under the ACTUAL interface instance only (per-port copies
  are views; `collect_iface_copies`).
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
- `for_stmt` in UHDM: `vpiForInitStmt`/`vpiForIncStmt` (not vpiStmt/
  vpiElseStmt) for init/incr, `vpiCondition` = condition, `vpiStmt` = body.
- `delay_control` values are NOT exposed via VPI in Surelog v1.87 — the
  `core::db` build recovers integer ticks or the delay expression spelling
  from source (`StmtKind::DelayControl { ticks, expression }`). Constant
  expression evaluation and timescale scaling happen in codegen.
- Generated C uses GNU statement-expressions `({ ... })` for select-LHS
  write-back (gcc/clang OK, not strict ISO C).
- Packed streaming expressions retain the operand width and are unsigned;
  streaming assignment targets retain typed component LHS expressions and
  explicit component widths so the RHS is evaluated once before unpacking.
  `inside` evaluates its selector and every scalar/range endpoint once, using
  wildcard equality for scalar items and ordinary inclusive comparisons for
  ranges.
- Dynamic arrays, queues, and associative arrays lower to distinct container
  IR and runtime storage; their packed elements retain arbitrary model width.
  The current vertical slice supports one resizable dimension, dynamic `new`
  and copy/delete, positional dynamic/queue assignment patterns, queue element/
  method operations, and integral or string-key associative access, delete,
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

Standalone scalar and packed `wire/tri` declarations use the same per-site
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
contribution slots.

The same bounded standalone-driver path supports `tri0/tri1` and
`supply0/supply1`. Pull defaults replace only all-Z bits after ordinary wire
resolution; X and conflicting active drivers remain X. Supply defaults dominate
ordinary implicit-strength drivers. Resolved cells start at their default before
processes run, while individual contribution slots start at Z. Explicit strengths
remain rejected for wired nets, pull/supply defaults, ports, and gates;
`trireg` charge storage is not implemented.
See `tests/sim_net_defaults.rs`.


## Initialization, generate scopes and interfaces

- True-net declaration assignments (`wire w = expr;`, including dynamic RHS)
  use the same event-driven `RunOnce`/`SensLoop` path as explicit continuous
  assignments. Reject dynamic true-net drivers reading unpacked arrays and
  unsupported resolved-net classes when sensitivity/resolution is unrepresentable.
- Variable initializers run in `main()` before processes, so a t=0 process
  write wins. Order: unpacked-array fills, scalar `reg` fills (`reg y = 0;`
  appears as `vpiNetDeclAssign`), then scalar variable fills (`logic l = 1'b0;`,
  `int x = 5;`, `logic [7:0] v = 8'ha5;` from `vpiExpr` → `Db::vars_init`).
  Fold RHS constants using collected parameters (`int y = P + 1;`); reject
  nonconstant variable initializers (`logic z = a;`).
- Generate-scope processes inline concrete genvar parameter values. Generated
  module instances retain per-iteration paths (`top.g[0].u`), parameters,
  signals, processes and links. Array-element port actuals (`.cnt(cnts[i])`)
  require compile-time constant indices, including elaborated genvars;
  reject dynamic array-element connections.
- Interfaces support actuals, per-port views, modport links and parameter-folded
  widths. Emit interface always/initial/always_comb bodies only under the
  actual instance; per-port copies are views, not cloned body processes.

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
  once when true. Constant true executes immediately; constant false spins
  until the runtime zero-delay guard trips. See the lowering loop above.
- Whole-signal `force sig = expr;` ignores procedural blocking/NBA writes while
  forced. `release sig;` restores the pre-force value without re-evaluating
  drivers changed during force (v1 approximation). Re-force changes the forced
  value but preserves the original saved value. Normal signal writes implement
  force/release so `@(sig)`/wait waiters wake.
- Procedural continuous `assign <variable> = expr;` / `deassign <variable>;`
  use a pre-scanned enable-guarded process per site. Deassign disables the
  driver, retaining the last value (`tests/sim_force.rs`).

## Hierarchical references and output

Hierarchical reads (`top.u0.sig`, any resolved N-part signal path) work in
expressions/display/monitor; whole-signal blocking/NBA writes target the
resolved instance, including collapsed inout-net drivers. Trailing write
selects (`top.u0.sig[3:0]`, `[2]`, `[3 +: 4]`) require constant integer
indices/bounds. Surelog v1.87 drops part-select bounds and retains constant
bit-select indices only in names: recover trailing selects from the node name/
source line; reject variable/expression indices or bounds. Hierarchical SELECT
reads still return the whole signal because the DB lacks the select.

`$monitor`/`$monitoron`/`$monitoroff` detect changes after each NBA commit;
only the most recent monitor is active. `$strobe` prints once with post-NBA
values for its timestep. `$write` uses display formatting without a newline.
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
continuous-assignment/sensitivity paths, and formatted/real conversion methods
such as `atoreal`/`realtoa` remain unsupported.

Packed Verilog string literals are unsigned integral byte vectors, with the
leftmost character most significant. Escapes retained by Surelog are decoded
  before the generated model-width limit is checked; an empty literal is one
  zero byte.
Assignments pad/truncate as packed values, and explicitly packed parameters
can initialize packed storage. SystemVerilog `string`-typed storage is
separate from packed Verilog byte vectors: basic declarations, assignment/
copy, casts, display paths, and bounded automatic function returns are covered,
while general string subprogram forms, ports, continuous-assignment/sensitivity
paths, and native formatted/real conversion methods remain unsupported. See the bounded
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
one bit. If Surelog loses the fill marker, source recovery requires an exact
two-character literal span; never reinterpret a folded compound expression
from its first token. Context widening must preserve model-sized division,
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

Surelog can incorrectly fold comparisons involving explicitly cast real
parameters. Lowering can reconstruct simple parameter/numeric-literal
comparisons from exact, admitted one-bit constant source spans. This bounded
recovery is not a general source-expression parser and must preserve lexical
shadowing; it must not treat identifier spellings as numeric literals.

`$rtoi` truncates toward zero into signed 32-bit storage (non-finite inputs
yield X; finite overflow wraps modulo 2^32). `$itor` preserves integral
arguments' packed width/signedness; a real argument first undergoes ordinary
rounded conversion to a signed 32-bit integer. Functions taking real values
accept implicit packed-to-real coercion. `$realtobits`/`$bitstoreal` and
`$shortrealtobits`/`$bitstoshortreal` reinterpret IEEE-754 representations;
the inverse bitcasts require exactly 64/32 bits, with X/Z positions treated
as zero. Typed parameters and constant declaration initializers use the same conversion rules.
See `tests/sim_real_conversions.rs`.

Reject real ports/links, arrays, function/task types, continuous/comb processes,
real event controls/wait conditions, monitor/strobe args, force/release/select/
case/repeat contexts, and bitwise/reduction/shift/concat/case-equality operations
before C compilation. `tests/sim_real.rs` pins support and rejection messages.

## Timescale

- Delays are timescale-aware (Verilator practice): the codegen parses the
  FIRST `` `timescale <unit>/<precision> `` directive of each source file
  (simple text scan, optional whitespace around `/`; units s/ms/us/ns/ps/fs
  with 1/10/100 multipliers) and scales every `#N` in that file by
  `N * unit / design_precision` before calling `llg_wait_time`.
  `$time` returns the current time in the calling module's unit
  (`llg_time() * design_precision / unit`), so `%t`/`%0d` displays show the
  unit-scaled time; `$printtimescale` prints the calling module's
  unit/precision.  Modules without a directive default to 1ns/1ps with a
  TIMESCALEMOD-style warning (once per file).
- The scheduler runs in design-precision ticks: the design precision is the
  FINEST precision across every module (default 1ns/1ps for modules without a
  directive), so 1 tick = design_precision ps.  The runtime itself stays
  timescale-agnostic (`llg_wait_time` receives already-scaled ticks), so no
  runtime change was needed.
- `core::db` retains raw `#N` ticks or source-recovered constant-expression
  spelling. Statement and intra-assignment delays accept resolved integer
  parameters, decimal literals with underscores, parentheses, unary `+/-/~`,
  arithmetic `+`, `-`, `*`, `/`, `%`, shifts `<<`/`>>`, and bitwise `&/^/|`. Evaluation
  preserves operand widths/signedness for its accepted subset, with at most
  128-bit operands and 256 parser steps; final ticks must fit a nonnegative
  `u64`. Mixed-width arithmetic and outer signedness changes affecting an
  already-computed operand are rejected until full context propagation exists.
  Nonnegative fixed-point literals and literals suffixed with `s/ms/us/ns/ps/fs`
  additionally work in procedural/intra-assignment delays, including enclosing
  parentheses. Exact decimal-rational arithmetic rounds to the calling module's
  precision (nearest, halves upward) before conversion to global scheduler ticks;
  check both evaluation and scaling overflow. Parenthesized scientific literals
  (bounded decimal exponent magnitude ≤38, including exponent underscores) and
  whole resolved real parameters also round locally; nearest declarations shadow
  outer parameters. Surelog rejects bare scientific delay syntax before lowering.
  Runtime values, arithmetic containing real/time literals or real parameters, based literals,
  logical/comparison/ternary and system-function forms remain unsupported.
  Exact time literals in runtime value expressions become module-unit realtime
  values after local-precision rounding, using source spans owned by `core::db`.
  Build the DB with `Db::build_with_source_files` and explicitly admitted physical
  sources (the CLI uses `CompileOut::frontend_source_files`); bare-handle
  `generate` uses ordinary `Db::build` and rejects suspect source-dependent
  values. Macros and unadmitted headers are rejected when provenance is missing.
  Reject parameter/declaration initializers and frontend-folded compounds
  containing time literals rather than accepting transformed integer payloads.
  Existing real-expression restrictions and the 64-bit local-tick bound apply.
  Sub-picosecond
  timescale precision still clamps up to 1 ps in the ps-integer representation.
  See `tests/sim_delay.rs`, `tests/sim_time_literals.rs`, and `tests/sim_time_values.rs`.

## Unpacked arrays and memories

- Storage: every array becomes a flat C array `sv4_t G_<path>_<name>[N]`
  (`N` = product of the per-dimension sizes `|left - right| + 1`); elements
  start all-X for four-state elements or zero for two-state elements (a loop
  in `main()` fills them, since a function call is not a
  valid static initializer).  Declaration initializers (`= '{…}`) — captured
  by `core::db` either on the array object's `vpiExpr` (`logic`/`bit` arrays)
  or as a `vpiNetDeclAssign` continuous assignment (`reg` arrays, which
  Surelog models as `array_net`s) — are applied in `main()` before any
  process runs; each pattern operand is a constant, in linear-index order.
- Indexed access: `mem[i]` (1-D), `a[i][j]` (N-D) and element-level selects
  `mem[i][3:0]` / `mem[i][2]` are supported on both the read and write paths.
  The linear index is row-major with the **leftmost dimension slowest**
  (matching Verilog); descending ranges (`[255:0]`) map `left` to offset 0.
- Out-of-range semantics (matching Verilog): an index outside the declared
  bounds, or with unknown (X/Z) bits, reads as the element type's default
  value (X for four-state, zero for two-state) and makes a write a no-op.
  Guard code uses `sv4_to_index_i64` to preserve index signedness and reject
  high-limb overflow, checks declared bounds before subtracting offsets, then
  computes the flat element address.
- Rejected with a clear message: dimension bounds that are not plain
  constants (Surelog keeps an implicit `[N]` size as `[0:N-1]` with an
  un-folded subtraction — declare `[0:N-1]` explicitly), array slices
  (`a[i]` on a 2-D array — partial indexing), indexed part-selects on an
  element (`mem[i][3+:4]`), non-constant declaration-initializer elements,
  and arrays wider than `LLG_MAX_WIDTH` per element.
- Non-blocking writes to a whole element record the element address and are
  committed in the NBA region like any other signal; a non-blocking write to
  an element *part* does its read-modify-write at record time (the value is
  computed from the element as seen when the assignment executes).
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
Positional and complete member-named assignment patterns lower fieldwise;
mixed, duplicate, omitted, default, and type-keyed pattern forms are rejected.
Anonymous whole-type copies fail closed when owned type identity is unavailable.
Reject unpacked aggregate nets/ports, nested aggregate or unpacked-array members,
unequal-width unpacked unions, tagged unions, declaration patterns, aggregate
subprogram formals/locals, compound assignments, and whole aggregates in scalar
expression contexts.
