# sim — Verilog/SV → C11 simulator

## Purpose

Compiles elaborated designs into C11 models that run as standalone
executables:

- `codegen.rs` — db → IR lowering (pure lowering; no string emission left).
  `generate(design)` builds the owned db (`core::db`) and lowers it into an
  `IrModel` (`generate_with_opts(design, &OptConfig::default())` delegates to
  the optimizer + backend); the lower_expr/lower_stmt/lower_lhs families and
  the process/link/function/init-step builders live here.  `GeneratedModel`
  carries a `pub design_name: String` plus the emitted `model_c` and warnings.
  The behavioral contracts below (inout nets, arrays, timescale, force/
  release, interface bodies, …) are decided here, at lowering time.
- `ir.rs` — the typed IR: signals/arrays/net-groups/functions/processes
  (`IrShape` = `RunOnce | Loop | SensLoop`) + init steps/spawns and
  `IrExpr`/`IrStmt` trees.  Sensitivity and read sets are computed at
  lowering and carried in the IR; the optimizer never recomputes wake
  behavior.
- `opt.rs` — conservative optimization passes over the IR, driven by
  `OptConfig { fold_constants, identities, prune_branches, unused_storage }`
  with `default()`/`none()` and per-pass toggling for bisection.  Constant
  folding reuses `core::elab::Value` math (X/Z-correct; shortreal casts fold
  through `f32` like the runtime; div/mod/pow only for known ≤64-bit
  operands); identity simplifications are shape-guarded; branch/case pruning
  follows strict provability rules (never prunes past non-const items, and to
  default only when ALL items are proven unmatched); unused-storage
  elimination uses an omit flag (no index remapping) with a read-collector
  covering processes, funcs, init steps, spawns, monitor eval fns, links,
  force targets, display args, wait sensitivity lists, and task-call temps.
- `emit_c.rs` — the C11 backend, consuming ONLY IR types; decoupled from
  `core::db`/`ffi`/`vpi` (enforced by `tests/emit_decoupling.rs` greps, same
  spirit as the repo's `unsafe`-confinement rule).  Naming conventions
  (G_/p_/D_ prefixes) and the `model.c` first-line header are unchanged.
- `build.rs` — the CMake-only model builder (`build_model_cmake`), the only
  supported build path, invoked automatically right after C emission.  It
  writes sources via the shared helper, generates a `CMakeLists.txt`
  (C11, Release default, exe under `<build>/bin/`, links `m`), runs
  `<cmake> -S <out_dir> -B <out_dir>/build [-G <generator>]
  -DCMAKE_C_COMPILER=<LLG_CC|$CC|cc> -DCMAKE_C_FLAGS:STRING="-O2 -Wall
  -Wno-unused-function [$LLG_CFLAGS]"`, then `cmake --build --config Release`.
  Generator selection: `CmakeBuildOpts.generator` (driver
  `--generator <backend>` via `build_model_cmake_with_opts`) >
  `$CMAKE_GENERATOR` passthrough > cmake's host default.
  `generate_model_sources` writes sources + `CMakeLists.txt` only (driver
  `--gen-only`).  Env vars: `LLG_CMAKE` (cmake program), `LLG_CC`/`CC`
  compiler chain, `LLG_CFLAGS` appended.  Missing cmake → actionable error
  naming install; flags containing double quotes are rejected;
  `cmake_available()` probes for a usable cmake once per process.
- Inout ports / tri-state nets: the parent and child nets of every inout port
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
- `rt/` — embedded C runtime (`include_str!`): `sv4_t` 4-state value ops +
  event scheduler (`llg_rt.c`), plus the libaco sources (`aco.c`/`acosw.S`)
  and the C self-test.  The self-test carries a deterministic vector table
  (`VECTORS[]` in `llg_rt_selftest.c`) whose expected values are generated
  from `core::elab::Value` by `tests/property_elab.rs` (`gen_c_vectors`,
  ignored test; regenerate with `cargo test --test property_elab
  gen_c_vectors -- --ignored --nocapture > /tmp/vectors.inc`), so the C
  `sv4_*` ops are cross-checked against the Rust 4-state math on identical
  inputs.
- `mod.rs` — the shared source-write helper `write_sim_sources` (writes
  runtime + libaco + extra sources into a build dir; consumed by the
  `build` module).

Driver: `src/bin/llg.rs` (compile → codegen(lowering → IR → opt → emit)
→ build → run).

## Requirements

- **No `unsafe`** (all UHDM access is through `core::db`, which is safe).
- **No direct VPI calls** outside the `generate` entry's db build.
- libaco is **not** a Rust dependency: it is compiled together with the
  generated C model at model-build time.
- Declaration initializers are supported and applied in `main()` before any
  process runs (a process writing the signal at t=0 overrides the
  initializer), in this order: unpacked-array fills, then scalar `reg`/`wire`
  fills (`wire w = 1'b1;`, `reg y = 0;` — Surelog models these as
  `vpiNetDeclAssign` continuous assignments), then scalar VARIABLE fills
  (`logic l = 1'b0;`, `int x = 5;`, `logic [7:0] v = 8'ha5;` — the init lives
  on the var's `vpiExpr`, captured by `core::db` in `Db::vars_init` and folded
  to a constant by the codegen; parameter references like `int y = P + 1;`
  resolve through the collected parameter values).  Initializers whose RHS is
  not a constant expression (`logic z = a;`) are rejected — v1 is
  constant-only.  Processes inside
  generate scopes are supported (genvar references inline to the gen-scope
  parameter values).  Module instances inside generate scopes are supported
  too: each per-iteration instance gets its own path (`top.g[0].u`), its
  parameters resolve per iteration, and its signals, processes and port
  links are emitted exactly like a regular child instance's.  Port
  connections to array elements (`.cnt(cnts[i])`) are supported when the
  index is a compile-time constant (as it is for genvar-elaborated selects);
  dynamic array-element port connections are rejected.
- v1 limits (documented in `codegen.rs`): widths ≤ 1024 bits
  (`LLG_MAX_WIDTH`, kept in sync with `llg_rt.h`); division/modulo/power
  still limited to 64-bit operands; X and Z are stored and displayed
  distinctly (`$display` prints 'x' vs 'z', `===`/`!==` compare them
  literally) and casez/casex wildcards are matched per LRM 12.5.1 — in all
  other expression contexts Z behaves as X (LRM 11.4.5) while identity/copy
  ops (mux with a known select, selects, resize, concat) carry Z through;
  functions and
  tasks are supported (recursion, defaults — including defaults referencing
  earlier formals — and delay-bearing tasks inlined at their call sites), and
  fork/join is supported in process bodies only
  (join / join_any / join_none, `wait fork;`, `disable fork;`, named forks;
  fork inside a function/task body is rejected); `wait (expr) stmt;` is
  supported (level-sensitive blocking: the condition is re-evaluated on
  changes of its read signals until true, then the body runs once; a constant
  condition executes immediately when true, and a false constant spins until
  the runtime's zero-delay guard trips); wait-bearing tasks are inlined at
  their call sites; interfaces are supported
  (actuals + per-port copies, modport links, parameter-folded widths),
  including interface body processes (always/initial/always_comb blocks
  inside an interface definition are emitted for the ACTUAL interface
  instance; the definition's processes are not cloned into the per-port
  copies, which are just views); `$monitor` /
  `$monitoron` / `$monitoroff` / `$strobe` are supported (monitor change
  detection after each NBA commit, strobe printing once with the post-NBA
   values of its time step, only the most recent `$monitor` active);
   `$write` shares `$display` formatting but does not append a newline;
   hierarchical references are supported on both sides of an assignment:
   READS (`top.u0.sig` — N-part paths whose final element resolves to a
   signal) in expressions and monitor/display arguments, and WRITES to a
   resolved per-instance signal (whole-signal blocking/NBA; the collapsed
   inout-net driver path applies automatically).  Hierarchical write
   targets may carry a trailing select (`top.u0.sig[3:0]`,
   `top.u0.sig[2]`, `top.u0.sig[3 +: 4]`) with constant integer
   indices/bounds only; Surelog v1.86's elaborated model drops part-select
   bounds and only keeps constant bit-select indices (in the object name),
   so the trailing select is recovered from the node name / source line —
   variable or expression indices/bounds are rejected with a clear error.
   Hierarchical SELECT reads remain whole-signal (the db does not capture
   the select); `%d` prints signed values as negatives (two's
  complement over the value's width) when the value's `is_signed` is set,
  and `-<unsized decimal literal>` (e.g. `-3`) is emitted signed per the
  LRM; `$dump*` / `$displayon` / `$displayoff` remain skipped with a
  warning; procedural `force sig = expr;` / `release sig;` are supported on
  whole signals only (while forced, procedural blocking and non-blocking
  writes to the signal are ignored; `release` restores the value saved at
  force time — drivers that changed while forced are not re-evaluated, a
  documented v1 approximation — and re-forcing updates the value while
  keeping the original saved value; force/release write through the normal
  signal path, so they wake `@(sig)`/`wait` waiters); procedural continuous
  `assign <variable> = expr;` / `deassign <variable>;` use a pre-scanned,
  enable-guarded process per assignment site (`deassign` disables the driver
  and the variable retains its last value; see `tests/sim_force.rs`);
  see the module docs for the full supported/rejected list.

- Real-number support: procedural scalar `real` and `shortreal` variables use
  companion `double` storage.  B6 supports constant initialization, real
  parameters, blocking/NBA assignment, mixed packed/real arithmetic, relational
  and logical operations, conditional expressions, casts, and real conditions
  in `if`/`while`/`for`.  Assignment to `shortreal` rounds through C `float`;
  implicit real-to-packed conversion rounds to the nearest integer (halves away
  from zero) and is limited to targets no wider than 64 bits.  Packed-to-real
  conversion accepts the full 1024-bit runtime width and treats X/Z bit
  positions as zero.  `$display` supports `%f`/`%e`/`%g`, including
  width/precision modifiers.  Deliberate v1 rejects include real ports/links,
  arrays, function/task types, continuous and combinational processes,
  real-valued event controls and wait conditions, real monitor/strobe
  arguments, force/release/select/case/repeat contexts, and
  bitwise/reduction/shift/concat/case-equality operations.
  `tests/sim_real.rs` pins the supported behavior and rejection messages.

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
- `core::db` keeps `StmtKind::DelayControl` ticks as the raw `#N` integer;
  scaling happens in the codegen.  Sub-picosecond (fs) units clamp up to 1 ps
  in the ps-integer representation; fractional delays (`#0.5`) remain
  unsupported.

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

## Interactions

- Below: `src/core/` (`compile` pipeline, `db`, `elab` values).
- The runtime is pure C: no Rust dependency; it is emitted into
  `target/sim/<design>/` and compiled with the model.
