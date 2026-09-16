# Structured owned-value emission: P05 incremental boundary

This renderer replaces nested allocating C expressions with ordered statements
and a result record (`Value`) carrying its code, width/sign/fill metadata and
owning slot. Real values are ordinary scalar C locals. Packed expression owners
live in procedure scopes; source-level packed locals use separate scoped cells.
There is no compiler cleanup attribute, statement expression, VLA, C++ destructor,
or simulation-lifetime temporary arena in this path.

## Implemented source paths (integration not yet validated)

| Area | Implemented path |
| --- | --- |
| Values | Constants, reads, packed/real arithmetic, casts, concatenation/repetition/stream reads, selected reads, ordinary inside tests, selected numeric system functions and mutation snapshots |
| Branches | Separate setup/cleanup for short-circuit and conditional arms; four-state packed mux merging; if/case and structured loops |
| Writes | Blocking, immediate/delayed NBA, selected masks, fixed arrays with captured guarded indices, ordinary resolved-net driver writes |
| Procedures | Numeric input/output/inout arguments, numeric automatic/static returns, return cleanup, registered arguments across yielding calls, uncaptured fork branches |
| Scheduling | Constant/runtime delays, stable signal/real/event waits, event triggers, stop/resume, completion and cancellation cleanup |
| Output | Packed/real and literal-string typed formatting, severity, waveform controls when enabled |
| Model storage | Owned numeric globals/static locals/returns, net drivers, fixed arrays and dependency markers; startup initialization and deterministic close |

Selection/call argument evaluation follows the existing typed IR ordering.
Opaque C fragments are never reparsed or treated as safe owners. Nonlocal forward
jumps may only exit or remain within a known lexical scope; backward unstructured
jumps and jumps into other scopes are rejected. Real-valued repeat counts are
also rejected rather than silently changing their conversion semantics.

## Deliberately rejected in this round

Class/object/container/virtual-interface model storage; native string objects and
native-object or ref formals; DPI and method calls; assertion/sampled-domain and
VPI system-call callbacks; true net aliases; event arrays; evaluated/filtered event
callbacks; captured or named fork/named activation scopes; force/inertial and
other unmigrated statements; streaming lvalues; enum/dynamic-cast expressions;
nonliteral string formatting; queued writes to automatic real cells; qualified
unique/priority diagnostics; and opaque `Verbatim` expressions.

Subprogram calls during pre-process declaration initialization are rejected:
this path cannot assume a legal coroutine context for a yielding task/function.
Unused functions and pre-functions are still checked/rendered conservatively;
an unmigrated unused definition can therefore reject the whole model.

Legacy lowerer sites that request detached C fragments remain rejected. Some
frontends therefore cannot yet reach this numeric path even when the underlying
operation is implemented here. These restrictions must be lifted by migrating
their typed ownership/capture contracts, not by removing guards or reviving old
by-value emitters. P05 is not complete.

## Lifetime rules

1. Borrow operation inputs, own every packed return, destroy consumed operands
   immediately and reuse only their now-empty descriptor slots.
2. A local cell has a separate registered scope. Ending lexical lifetime drops
   the scope's lexical reference; a pending NBA/clocking record retains that
   cell's descriptor allocation until commit/discard. This reference count
   protects cell identity, not shared/COW packed values.
3. Procedure roots preserve arguments/results across a yield. Early return
   clones the return cell before scope unwinding; queued selected writes can
   still need the original cell. Process cancellation unwinds registered scopes
   without resuming the coroutine or depending on C stack unwinding.
4. Output targets/indices are captured before the callee; call arguments and
   copy-out temporaries survive suspension. Native real locals cannot be NBA
   targets until equivalent descriptor retention exists for them.
5. Runtime queues and VPI observers are torn down before model storage. A
   suspended model remains live until resumed or explicitly closed. Startup and
   close are repeatable; double start is rejected. Concurrent model instances
   remain unsupported by the existing process-global runtime.

## Responsibility map

- `owned.rs`: result records, reusable temporary slots, binding/assignment casts.
- `expressions.rs`, `control.rs`, `system.rs`: expression evaluation and branch setup.
- `stores.rs`: captured lvalues, masks, guarded array access, mutations.
- `calls.rs`, `events.rs`, `formatting.rs`: call/wait/output ownership boundaries.
- `statements.rs`: lexical/loop/control-flow cleanup.
- `model.rs`, `model/`: procedures, persistent storage, initialization and host API.
- `tests.rs`: Rust structural tests and an opt-in numeric-IR-to-C execution test.

## Required verification

The delivery ran native C runtime and sanitizer probes, not this Rust emitter.
No Rust compiler was available. Delimiter/symbol inspection is not compilation.
Run the Rust tests and numeric model test first, then extend generated regressions
for recursion, side-effect ordering, arrays, all return/jump edges, cancellation,
stop/resume, output callbacks and failure cleanup. Old fragment/golden tests need
migration; their previous expectations are not acceptance evidence for this ABI.

```sh
cargo test --lib --no-default-features sim::emit_c::owned::tests
cargo test --lib --no-default-features structured_owned_model_executes_numeric_loop -- --ignored
```

The opt-in test starts from typed numeric IR, not a Verilog frontend. Full HDL
integration, sanitizer-aware fiber instrumentation, macOS/Windows execution and
performance measurement remain separate acceptance gates. No speedup is claimed.
