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
- Expression widths: constants, parameters, signals and concat/
  replication results are all checked against `LLG_MAX_WIDTH` (1024);
  division/modulo/power operands are additionally limited to 64 bits —
  silent truncation in `sv4_concat` is a real bug, keep the checks.
- `for_stmt` in UHDM: `vpiForInitStmt`/`vpiForIncStmt` (not vpiStmt/
  vpiElseStmt) for init/incr, `vpiCondition` = condition, `vpiStmt` = body.
- `delay_control` values are NOT exposed via VPI in Surelog v1.87 — the
  `core::db` build recovers integer ticks or the delay expression spelling
  from source (`StmtKind::DelayControl { ticks, expression }`). Constant
  expression evaluation and timescale scaling happen in codegen.
- Generated C uses GNU statement-expressions `({ ... })` for select-LHS
  write-back (gcc/clang OK, not strict ISO C).


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
skip. String-typed signals/parameters remain unsupported.

Packed Verilog string literals are unsigned integral byte vectors, with the
leftmost character most significant. Escapes retained by Surelog are decoded
before the 1024-bit limit is checked; an empty literal is one zero byte.
Assignments pad/truncate as packed values, and explicitly packed parameters
can initialize packed storage. SystemVerilog `string`-typed storage remains
unsupported. See `tests/sim_packed_strings.rs`.

## Values and real numbers

Keep `LLG_MAX_WIDTH` (1024) aligned with `llg_value.h`, including constant,
parameter, signal, concat and replication checks; div/mod/pow remain ≤64-bit.
X/Z remain distinct for display, literal equality and casez/casex matching
(LRM 12.5.1); Z behaves as X in other expression contexts (LRM 11.4.5), with
identity/copy operations preserving it. See runtime value contracts.

Wildcard equality (`==?`/`!=?`) treats only RHS X/Z bits as wildcards after
common-width and signedness conversion. A known mismatch wins over an unknown
LHS bit at another position; otherwise an unmasked LHS X/Z yields X. Real
operands are rejected. `tests/sim_wildcard_eq.rs` compares optimizer variants.

`$countones`, `$onehot`, `$onehot0`, and `$isunknown` accept one packed
integral expression up to `LLG_MAX_WIDTH`. One counts ignore X/Z positions;
`$isunknown` detects either state. Count results are signed 32-bit integers,
and predicates are unsigned one-bit values. The argument evaluates once;
optimizer read collection and combinational sensitivity retain its dependencies.
Real operands are rejected. See `tests/sim_bit_queries.rs`.

Procedural scalar `real`/`shortreal` use companion `double` storage. B6 supports
constant initialization, real parameters, blocking/NBA assignment, mixed
packed/real arithmetic, relational/logical operations, conditionals, casts,
`if`/`while`/`for` conditions and display `%f`/`%e`/`%g` width/precision.
Shortreal assignment rounds through C `float`; real-to-packed rounds nearest
(halves away from zero) for targets ≤64 bits. Packed-to-real accepts all
1024 bits, treating X/Z positions as zero.

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
  check both evaluation and scaling overflow. Runtime values, real parameters,
  scientific notation, arithmetic containing real/time literals, based literals,
  logical/comparison/ternary and system-function forms remain unsupported.
  General time-literal value expressions are not implemented. Sub-picosecond
  timescale precision still clamps up to 1 ps in the ps-integer representation.
  See `tests/sim_delay.rs` and `tests/sim_time_literals.rs`.

## Unpacked arrays and memories

- Storage: every array becomes a flat C array `sv4_t G_<path>_<name>[N]`
  (`N` = product of the per-dimension sizes `|left - right| + 1`); elements
  start all-X (a loop in `main()` fills them, since a function call is not a
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
  bounds, or with unknown (X/Z) bits, reads as X and makes a write a no-op.
  Guard code is emitted inline (`sv4_to_i64`/`sv4_is_unknown` on each index,
  a per-dimension offset/range check, then the flat element address).
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
