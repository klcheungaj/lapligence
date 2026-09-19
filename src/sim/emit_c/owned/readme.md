# Structured owned-value emission: P05 incremental boundary

This renderer replaces nested allocating C expressions with ordered statements
and a result record (`Value`) carrying its code, width/sign/fill metadata and
owning slot. Non-addressable real expression results are scalar C temporaries.
Addressable real locals use registered, heap-backed `double` payloads: other
coroutines must not write into a suspended coroutine's shared C stack. Packed
expression owners live in procedure scopes; source-level packed locals use
separate scoped cells.
There is no compiler cleanup attribute, statement expression, VLA, C++ destructor,
or simulation-lifetime temporary arena in this path.

Feature availability and the remaining migration gates are maintained in
[the simulator feature checklist](../../../../docs/sim_features.md#dynamic-value-migration-acceptance-boundary).
Do not infer full HDL support from a runtime component or bypass a feature guard.
Selection/call argument evaluation follows typed IR ordering; opaque C fragments
are never reparsed or treated as safe owners.

## Lifetime rules

1. Borrow operation inputs, own every packed return, destroy consumed operands
   immediately and reuse only their now-empty descriptor slots.
2. A packed local cell has a separate registered scope. Ending lexical lifetime drops
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
- `calls.rs`, `events.rs`, `formatting.rs`: call/address/output ownership boundaries.
- `captures.rs`: evaluate numeric owners and typed handle snapshots before publishing activation frames.
- `event_waits.rs`: snapshot wait addresses and publish per-field context references.
- `model/callbacks.rs`: captured numeric branches and read-only evaluator entry points.
- `statements.rs`: lexical/loop/control-flow cleanup.
- `model.rs`, `model/`: procedures, persistent storage, initialization and host API.
  The model initialization frame admits legal zero-time user calls needed by
  declaration initializers; timing-bearing callees remain impossible because
  functions cannot contain timing.
- `assertions/`, `assertions.rs`, `assertion_tasks.rs`: sequence-local bindings,
  sampled predicates/history, registration, assertion controls and deferred actions.
- `clocking.rs`: issue-time clocking operands, selected masks and borrowed runtime calls.
- `qualifiers.rs`: diagnostic candidate evaluation and first-selected branch execution.
- `native.rs`, `strings.rs`, `objects.rs`: registered native payloads, owned byte-string
  expressions and typed chandle/process-handle operations.
- `containers.rs`, `containers/`: ordered container operands, borrowed string keys,
  callback item snapshots and destination-aware runtime calls.
- `input.rs`: captured file-input/plusarg targets, owned source/format snapshots.
- `native_access.rs`, `references.rs`, `mailboxes.rs`: use-site member resolution,
  registered reference descriptors, synchronization handles and message transfers.
- `native_tasks.rs`: owned text consumption, numeric queue/random calls and VPI arguments.
- `pure_calls.rs`: bounded callback inlining of automatic numeric functions;
  each expansion renames its internal labels and reserves its escaping result
  in the caller scope before creating private callee storage.
- `streaming.rs`: snapshot the packed RHS, capture destinations and reject
  insufficient source bits before publishing staged writes. Fixed-selector
  loop indices use registered packed temporary slots and are destroyed on
  each iteration. Bounds errors retain in-range writes and mark failure.
- `tests.rs`, `tests/batch120.rs`, `tests/native_values.rs`: Rust structural and
  numeric-IR-to-C regressions. `tests/native_boundaries.rs` adds batch 5 contracts.
  `tests/review_regressions.rs` checks forwarded reference resolution, native
  reference-bit indices and stable, lexically owned real local addresses.
  `tests/group1_repairs.rs` checks inline label namespaces, caller-scope real
  results, streaming index ownership and pre-publication source-size checks.

## Verification

[The test guide](../../../../tests/readme.md#dynamic-ownership-validation) separates
Rust structural checks, typed-IR-to-C execution, public HDL regressions and native
C component probes. In particular, a hand-authored C output-shape fixture is not
an execution of this emitter. Context tests check one retain per descriptor field,
including shared eval/condition frames; array tests check index cleanup and inert
invalid handles. Side-effect-capable evaluator expressions remain fail-closed.

```sh
cargo test --lib --no-default-features sim::emit_c
cargo test --lib --no-default-features structured_owned_model_
cargo test --locked --no-default-features --test sim_dynamic_ownership
```

## Third repair batch: source implementation boundary

The batch targets 120 named failures from `nextest2.log`; it is not a record of
120 passing tests. Rust formatting, compilation and execution were not performed
for this batch. Its C checks are syntax-only runtime/API checks, not HDL runs.

Alias visible cells have independent packed owners, canonical dependency addresses
and explicit start/close handling. Runtime temporaries spanning alias publication
or stochastic output callbacks belong to registered cleanup scopes. Sequence-local
match mutations use private sequence storage, not public signal notifications.
VPI argument arrays borrow registered expression slots for the duration of a call.

Inline event formals snapshot event object identity and are not emitted as numeric
C procedures. Concrete interface metadata may describe borrowed member addresses;
this does not enable dynamic virtual-interface handle storage. The callback inliner
accepts automatic numeric expression-only functions with value formals whose bodies
contain no timing, scheduler, or externally visible writes: automatic locals, loops
and nested eligible calls are allowed as long as every write stays activation-local.
Other callback calls still fail closed. Packed streaming destinations are captured before any store.

This third-batch boundary is historical; the current acceptance boundary is
maintained in the linked feature checklist. New combinations can expose a later
guard. Historical feature markers remain subject to public HDL acceptance.

## Native values and container boundary

Native strings and retained process handles use object-bearing registered scopes.
The payload address is stable through suspension; its destructor runs on lexical
exit, cancellation or teardown. A callee copies string inputs, and a native call
reserves the return owner before its argument-cleanup mark. String output values
are copied back only after cancellation checks. A string reference borrows an
address, not the pointed-to allocation. Packed references now use typed descriptors and registered reference scopes;
queue-element references retain identity across relocation/removal.

Container calls borrow registered packed operands and key strings. Consuming string
arguments are detached only at the final call boundary. Method callbacks return
independent packed owners and compute all results before publishing any. Model
containers have initialized dependency markers and explicit teardown. Nonlocal
callback cleanup is a runtime responsibility too: converted queue/associative
operands and normalized keys are freed before notification.

File scans and plusarg operations capture destinations and retain source/format
snapshots until the runtime returns. Success-only plusarg copy-back and runtime
host-command permissions remain intact. See the test guide for the distinction
between unexecuted Rust structural regressions and executed C runtime probes.

## Native access, references and synchronization

Class constructors and dynamic member receivers are typed IR recipes rather than
C fragments. Resolve each receiver once at its use site, before call arguments or
publication. The validated nominal class layout uses a common C object header and
individually typed fields; derived/base access does not cast unrelated structs.
The model registry retains class objects until close, including cyclic graphs;
this is model-lifetime ownership, not garbage collection. Teardown follows runtime
callbacks and process cleanup. Packed field payloads use their own exact widths.

Reference calls construct descriptors from typed lvalues/reads, never the legacy
`addr` fragment. Whole/selected variable descriptors borrow storage; queue references
retain a cell that survives relocation and becomes detached after removal. Register
reference scopes on the value-owner unwind stack so cancellation releases cells.
Const-ref expression-only callbacks read current actual values without publishing
callee-local writes; broader callback functions still fail closed.

Semaphores and mailboxes use runtime-owned identities. Captured semaphore handles
copy that identity, not the object. Construct message payloads only after evaluating
all user operands. A consuming mailbox delivery removes its queue node before
publication; peek clones its snapshot. Delivery snapshots and converted packed values
remain registered when callbacks reenter, kill a receiver, or terminate the writer.
Signal publication pins an automatic packed destination until callbacks return.

DPI inputs are borrowed. Snapshot every foreign string result before the first
copy-out, retain packed results through publication, and check cancellation before
copying outputs. Mixed packed/container streaming captures all target selectors and
source pieces before writing. Container unstream helpers defer notification until
intermediate owners have been destroyed and the complete assignment is installed.

These are source changes for 60 previously unselected input failures, not a claim
that those tests pass. No Rust formatting, build or execution was performed for
batch 5. `native_boundaries_probe.c` separately exercises runtime reference cleanup,
reentrant mailbox delivery, receiver cancellation and streaming callbacks. Its C
results are not public HDL or actual Rust-emitter validation.

## Fixed-stream selector scope

The current stream emitter still captures all selectors before publishing any
stream destination. This repair batch adds size checks to that existing subset;
it does not implement `with` selectors that depend on values unpacked earlier in
the same assignment (IEEE 1800-2009 11.4.14.4). Do not use this staging policy as
a general language rule or claim full streaming conformance from these tests.

## Captured packed-element selections

The active emitter converts `IrElemSel::PackedChain` into a local
`sv4_select_plan_t`. It evaluates each selector once into a registered packed
owner, refines the plan, then releases that owner. The plan contains only integer
coordinates and widths: it owns no values and contains no destination pointer.
Reads apply the plan to an independent whole-element snapshot, supplying an
unpacked element's default first when its unpacked index is invalid. Inner
out-of-bounds positions then become X; two-state assignment conversion remains
a separate step.

Blocking and compound writes modify only the plan's valid interval. Nonblocking
writes snapshot the corresponding full-width value and mask at issue time and
use the existing masked-NBA commit path. No pointer to a local plan survives a
suspension or is stored in the NBA queue. Synchronous file-input targets may use
`LLG_REF_PACKED_PLAN`, whose `retained` field borrows the plan only for that input
call. Such descriptors are not queue-cell reference owners, are not registered
as retained references, and must never be forwarded as subroutine `ref` actuals.
The guarded legacy expression-fragment emitters still reject this typed form;
they are not a fallback for the active owned path.

The component probes in `tests/runtime_value_storage/packed_selection_*.c` use
production runtime helpers, not output from the Rust emitter. Structural Rust
checks and HDL regressions in `sim_group1_repairs` remain distinct acceptance
layers.

## Packed formal member bindings (R09/R14)

`IrLhs::PackedSelect` preserves an activation-relative root and normalized member
selection steps. WholeRef/Ref roots are resolved through the current local/formal
binding map, including inline callback overrides. There are no synthetic
`__llg_abi_formal_leaf` model signals or optimizer exceptions. Inputs use the
existing private value owner; member writes must never mutate the borrowed C
input descriptor. Reference member writes clone/read the referenced parent,
apply a selection plan, and publish through the original descriptor immediately.
Output/inout writebacks freeze their member indices once and retain copy-out at
return. Const references and NBA into activation storage remain errors.

Two-state member read conversion happens before subsequent selects add missing
X positions. Read-modify-write converts only the selected result at the store
boundary and preserves other four-state fields of the parent. Every index value
and temporary owner uses the existing registered lifecycle and cleanup paths.
`packed_formal_probe.c` is a hand-written runtime contract transcription; it is
not generated-C acceptance. The file-backed formal suite and owned-emitter unit
tests must still run in the actual frontend/Rust environment.
