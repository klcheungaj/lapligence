# Lowering: processes, dependencies and initialization

Continuous assignments and comb/latch/implicit-sensitivity processes evaluate at
zero, then wait on RHS/body reads, not their LHS base. A zero-delay continuous
driver whose target storage, or another view of the same alias network, is
also an operand snapshots those operands and repeats in place while one
changed (`collection/continuous_feedback.rs`); a nonconvergent loop stops at
the process step limit. Combinational sensitivity
includes called-function reads and excludes written storage (a whole-array
read keeps its one contents marker above `PRECISE_EXCLUSION_CELLS` cells, see
its docs); plain
`@*` retains call-site behavior. String variables and string record members
contribute their change marker. Startup spawn order (T0-D2): comments at the
spawn ordering in `lowering.rs`; SystemVerilog static initializers are no
change (T0-D1). Carry exact always-kind and typed writes through validation;
writer, timing and flip-flop violations reject independently of lint. Ordinary
`always` remains a procedural loop with cooperative, source-located back-edge
budgets, not a comb/run-once replacement.

Preserve program-instance origin and Reactive launch (initials, generate-scope
initials and continuous assignments of a program), rejecting prohibited program
members (also inside program generate scopes) before lowering; `$exit` admission
requires program-process context.
Preserve source-origin classification and declaration lifetime rather than
qualifier spelling. Defaults precede initialization and processes: ordinary nets
start Z, arrays/scalar variables receive their typed fills/initializers, and
resolved defaults/delayed driver X contributions retain their separate phases.
A time-zero process write must win over initialization. Static locals initialize
once in hidden model storage; automatic locals initialize per lexical entry,
including inherited lifetimes and loop/block scopes.
Subroutine calls always use the typed `IrFunc`, timed or disabling included;
there is no call-site expansion (removed by SIM-009-A03). A `ref` formal read by
an event control binds a whole-signal actual statically: each distinct actual
signal gets one cloned task (`task_specialization`, lowered after all call
sites); otherwise the typed body follows whatever its descriptor names
(`IrDependency::RefFormal`): a variable, fixed-array element, class property,
container element (its container's contents marker), member view or unpacked
aggregate (range-precise leaves). Selects, qualifiers, named-event lists, `@*`
and controls reading a subroutine container are process-evaluated. A task
waiting on a native record `ref` formal is a template reached only through
record specializations (`waits_on_record_ref`). String and handle `ref` formals
wait on their actual's marker (`IrDependency::NativeRefFormal`); each call
checks its actual has one (`native_ref_actual_observable`). Keep those cases in
that one predicate and record new ones there. A fork-shared string keeps a
marker only when `native_observed` finds a wait that can observe it; waits on
fork-shared strings and handles use `IrDependency::SharedNative`.
An event evaluator reads fork-visible automatics through its context slots:
`event_context_with_aliases` maps the declaring subroutine's local (which
array-element reads name) to the slot, and a joined branch's borrowed capture
stays borrowed even when a detached fork elsewhere shares the cell.
Fixed aggregate projections of static function locals use that persistent signal
as their read/write owner, including tagged-member guards; activation locals do
not name the persistent cell.

Collect parameters and concrete genvars with each generated path.
Subroutine names, prototypes and bodies traverse both generate arrays and
generate scopes recursively, including nested conditional/loop scopes and their
module instances. Fork references to input event formals resolve through typed
event captures before the enclosing task's formal bindings.
Replay deferred initializer calls/child references only after storage/prototypes exist, with
recursion depth reset; fail the whole replay on error. Declaration-call and member
default initialization finish before SystemVerilog processes start, never lazily
on first call. Reject unavailable lifetime/provenance or unrepresented native/
resizable initialization rather than fabricating storage.
Static initializers form one schedule (`lowering/initialization/order.rs`): an
initializer runs after every static declaration its value reads, directly or
through called functions (including their static locals), and otherwise keeps
the declaration order reserved at collection, so deferred and late-lowered
initializers keep their declaration slot. Package and `$unit` parameters are
collected before any module initializer folds them. Descriptor-backed fixed
arrays (module, package, function-static and block-static) initialize through
the typed pattern transport as `IrInitStep::Execute` in SystemVerilog and as an
active-region process in Verilog-2001; automatic descriptor locals declare and
initialize on each entry. Class-handle, semaphore, mailbox and string
initializers (module, package, `$unit`, static subroutine and class-method
locals, static class properties) and static native-record locals are
`IrInitStep::Execute` statements in the same schedule, never run-once
processes or first-call code; a handle initializer carries its instance as
`random_scope` so created objects seed from that instance's initialization RNG
(SV 18.14.1). `initialization/effects.rs` rejects a static initializer whose
calls (or constructor) reach a fork, nonblocking assignment or nonblocking
trigger (SV 13.4.4); `collection/const_writes.rs` rejects modifying built-in
methods on `const` declarations (SV 6.20.6, `Db::is_const_variable`). A name
redeclared in a package or `$unit` scope (a frontend warning) rejects instead
of aliasing storage.

Input links evaluate in the parent scope; selected outputs preserve untouched
bits. Constants/omitted defaults run once; explicit opens ignore defaults. Whole
packed/fixed aggregate refs retain canonical storage and member views without
copy links. Scalar real links use doubles and notify changed dependencies.
Interface/modport bodies use the actual Slang-bound instance; a simple modport
port resolves to its same-named interface member. Connection indices must be
elaborated constants (including genvars), except a variable output target: it is
an implied continuous assignment (SV 23.3.3.2) whose link re-evaluates its
selectors. The local Slang patch admits runtime selects only there and on
continuous assignments to variables (SV 10.3, A.8.5), whose process also waits
on its target selectors (`continuous_assignment_reads`) and whose writer set is
the longest static prefix (`add_continuous_lhs_write`). Ref, net, inout and gate
terminal connections keep constant selects. A ref actual that selects one leaf of a
record lowered as a concatenation binds to that leaf
(`collapse_concat_reference`), never to an unnamed projection. Descriptor-backed fixed-array ports and
nested member-array outputs reuse the procedural fixed-array assignment owner
(`FixedArrayCopy`/`FixedValueAssign`); never expand a descriptor port per cell.
Precollect hierarchical actual dependencies per instance.

Variable-continuous conflict analysis follows canonical intervals and counts
ordinary assignments/declaration initialization, not force/release/deassign.
Record and hierarchical member selects keep their declaration's storage class.
Constant-row write precision: `PRECISE_ROW_WRITE_CELLS` and
`array_row_write` docs (`collection/dependencies.rs`); `ir_process_writes`
widens the interval before the IR.
Output ports connected to variables are continuous drivers: constant rows of
dense arrays drive their cells, runtime selects drive the longest static prefix,
constant rows and contiguous constant slices of descriptor storage drive one
cell interval (a slice whose cells are not one interval stays unregistered
rather than a false whole-array conflict); keep disjoint
writers legal and preserve original read sensitivities separately. Zero-delay
continuous drivers of whole descriptor arrays or constant descriptor rows use
the fixed-array assignment owner, never a flattened packed driver.
The always-family single-writer rule counts ordinary assignments only;
force/release and procedural `assign`/`deassign` are overrides another
process may apply. always_ff admits event triggers and these overrides; only
event-control count, blocking timing and forks are restricted.
Hierarchical structural driver identity includes owner, source and group.
