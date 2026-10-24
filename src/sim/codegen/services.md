# Lowering: randomization, coverage, time and native services

## Constrained randomization

`randomization/` lowers `randomize()`, `std::randomize()` and the mode methods
(IEEE 1800-2009 clause 18; scope and decisions S29-D1..D10 and S30-D1..D11 in
[docs/lrm_decisions.md](../../../docs/lrm_decisions.md)). Each call site and
dynamic class case owns one static `IrRandProblem`; constraint expressions are
lowered by the ordinary expression path while `RandLowering` maps every random
variable to a `_llg_randvar_<k>` placeholder (`lower_expr`, `class_field_cell`,
receiver and `this` hooks), and `ProblemBuilder` turns the placeholder-bearing
IR into solver nodes. Subtrees without placeholders become state leaves read
once before the solve; logical operators keep their structure so guards
(18.5.12) can suppress an erroring leaf. Never let a constraint read random
storage directly: `check_direct_reads` is the backstop, and a constraint a
problem cannot express is a located error, never silently dropped.

`rand_mode`/`constraint_mode` state is hidden one-bit two-state storage per
random property and per root constraint block (overriding blocks share the
root's bit; static members use a static signal), invisible to `%p`. The
dynamic class is resolved through `DynamicCast` cases, most derived first; the
most-derived user `pre_randomize`/`post_randomize` (declared line != 0) runs
before modes and state are read and after the commit. A value-form randomize
is hoisted into `rand_prelude` only where it runs exactly once per statement
evaluation; short-circuit/conditional operands and loop conditions reject.
Commit-after-solve (18.6.3): `randomization.rs` module docs.

Arrays, `foreach` and `rand` object members make a problem a template with
units (`templates.rs`, decisions S30-D5..D11): unit 0 is the randomized object
or `std::randomize` scope, each other unit a class an object reached through a
`rand` handle can have; variables, arrays and handle slots are problem-wide,
one per property declaration. `template_forms` runs before a constraint
expression is lowered and registers array selects, `size()`, reductions
(`item`, `item.index`), `inside` an array, members and null tests through
`rand` handles as `_llg_randvar_t<n>` placeholders that `random_placeholder`
returns; `check_template_uses` rejects a form lowered another way and a random
array read directly. The generated code walks the instances the runtime
registers (`pre_randomize`, variables, blocks, leaves, arrays, links), then
commits arrays and member objects and runs `post_randomize` in registration
order (S30-D7). Problems without these forms keep the flat SIM-029 shape.

## Functional coverage

Covergroup lowering and its synthetic class layout: `coverage.rs` module docs.
Only groups that a `new` or an instance method call names lower; the class is
`class_nodes[group]`, field 0 is the record chandle, input fields are
`class_fields[formal]`. While a sample body or an
option lowers, `coverage_scope` makes argument reads go through the
covergroup object and `coverage_ref_actuals` maps each `ref` formal to the
variable bound by that variant; the owner is the method receiver, so owner
properties lower as ordinary class fields. Bins are resolved here (19.5.6 on
the operand of the frontend's conversion to the coverpoint type) into sorted
disjoint key segments; signed keys flip the sign bit. Transition bins become
compile-time automata in `coverage/transitions.rs`, whose `Tables::consume`
mirrors the runtime `llg_cov_consume`; it also sizes `[]` arrays, counts
coverable sequences and finds `detect_overlap` pairs within fixed budgets
(exceeding one is a located llg-limitation error). Cross select expressions
become DNF boxes over value-bin ranks (`cross_select`), at most
`MAX_SELECT_BOXES` per bin. Options are checked at their 19.7 levels
(`check_option`); type options must be constants. A clocking event on static
storage lowers to one sampler process spawned before user processes. An event
reading an argument or owner property becomes a waiter per instance
(`emit_covergroup_waiter`): a `CapturedBranch` helper of the sample method
whose only capture (slot 0) is the covergroup object, lowered with that object
as `coverage_scope` and the owner field as class receiver, and started by
`IrCoverageStmt::Watch` in `new`; `ref`-variant events dispatch on the bind
field. `type_option.strobe` turns the sample into a Postponed request.
Block events are injected into the named subroutine body in
`collection/function_bodies.rs` (a `return` there becomes a store and a jump
to the label before the end samples, `EmitCtx::coverage_end_exit`) and into
named blocks in `statements/dispatch.rs` (the event names the block's label
scope, resolved by `named_block_statement`); `check_coverage_block_events`
rejects any target that never reached either.
Diagnostics must stay located (`coverage_error`), and every coverpoint read
must stay an IR operand so `opt/passes/storage.rs` sees it.

## Time, output and native services

Use the owned units/precision of the nearest module instance, package, `$unit` or
class-declaring scope (`Db::declaration_time_scale`) and typed `IrDelay`; round complete real
delays once locally before checked scaling. Packed X/Z delay becomes zero, negative
packed values convert to unsigned 64-bit time, and finite negative real delays
convert after local rounding (constants take the runtime conversion); reject
nonfinite real delays and tick overflow. `timescale.rs` owns tick representation; keep the runtime timescale-
agnostic. `$time`/`$stime` round to calling units with halves upward; `$realtime`
keeps fractions. `%t` carries physical units through runtime `$timeformat`;
`$printtimescale` reports its instance/`$unit` operand, or the scope containing
the call, in the SV 20.4.1 format. Code-bearing packages and `$unit` join the
design tick. A `$unit` without `timeunit`/`timeprecision` has no recorded scale
(a `timescale directive does not set it, SV 3.14.2.3): it uses unit 1ns and the
finest declared precision capped at 1ns, or 1ps when nothing is declared
(decision F2-D2). Frontend owns inheritance.

Real/shortreal use typed doubles and f32 rounding at shortreal writes; packed/real
conversion rounding: `lowering.rs` module docs (at actual width).
`$rtoi` truncates to signed 32-bit (nonfinite X, finite overflow modulo 2^32);
`$itor` preserves packed width/sign and rounds real input through signed 32-bit.
IEEE bit-reinterpretation inverses require 64/32 bits and zero X/Z. Typed constants
share conversions. Validate all 21 Table 20-4 math functions, evaluate each argument
once and retain libm domain/nonfinite behavior. Do not infer real expressions from
source or bypass context-specific rejection guards.

Parse display formats during lowering (`objects/format_text.rs`, shared by every
display, file, string and severity task); `%d` retains signedness and `$write` omits
newline. Arguments stay typed: never re-parse rendered text. Literal string
arguments start format segments, `%m` comes from `format_scope` (nearest named
block, named fork (its target block), subroutine or generate block; in class
methods the class below the scope from `Db::class_declaring_scope`, S99-D10; an
`extern` body is named once, not also by its prototype, CD-D2), and non-`%p`
conversions of unpacked values
or handles reject. `%p` lowers to `IrStringExpr::Pattern` (`objects/patterns.rs` docs); the
runtime walker's limits are in `rt/scheduler/patterns.c` docs. Monitors/strobes run after settled Active/Inactive/NBA work; the latest `$monitor`
(plus any number of `$fmonitor` lists), registration/enabling forces a report,
time-only arguments do not retrigger. A deferred report in a static subroutine may
name model storage (static formals and locals included); activation-bound storage
(`activation_bound_ref`) and class methods reject. A monitor whose arguments read
handle-selected storage or `%p` a class object sets `IrStmt::MonitorSet::poll`:
the runtime then re-evaluates it at every settled slot (`LLG_MONITOR_READ_POLL`).
Deferred callbacks need owned environments. Waveform controls preserve source/depth
identity against the fixed catalog; a `$dumpvars` argument naming a variable must
match waveform storage after lowering (S47-D9), and waveform names drop the loop
generate construct level (`tb.g[0]`, S47-D10); omit waveform/libfst when unused, reject
`$dumpports`, warn/skip `$displayon`/`$displayoff`. Packed string literals remain
unsigned MSB-first byte vectors (empty is one zero byte), with normal pad/truncate;
SystemVerilog strings remain separate native owners.

Sampled-value functions intern one `IrSampledClock` per lowered clocking event
and gate and one `IrSampledDomain` per (clock, sample expression), keeping the
deepest `$past` depth, so overlapping calls share history. A single direct
packed-signal edge is an `Edge` clock; any other legal event list is an `Event`
clock ticked by a synthetic `WaitEvents` + `SampledClockTick` process spawned
before other time-zero processes. `iff` and the `$past` gate read current
values. Classify argument types from the Db before lowering: §16.6.1-excluded
types are illegal, unpacked/queue values are explicit unsupported; never coerce
a non-integral value into packed bits (reals use their exact 64-bit image).

Concurrent assertions use dedicated instances, immutable sampled predicates,
explicit clocks/disable/control metadata, per-attempt/thread locals and ordered
match-item effects. Preserve Slang actual/default expansion. Clock flow and leading-clock registration: `assertions/clocks.rs` module docs
(parentheses come from `Db::is_parenthesized_assertion`). A clock on one
direct packed-signal edge stays an edge clock; every other clocking event
(event lists, `edge`, value changes, bit selects, clocking-block events, `iff`
gates off the leading position, `$global_clock`) is a planned pulse clock: a
hidden one-bit signal pulsed at most once per time step by a synthetic
process (`IrStmt::AssertionClockPulse`, `llg_assertion_clock_pulse`), shared
by equal event lists. `$inferred_clock`/`$inferred_disable` defaults resolve
at the instance (16.15.7). Locals are initialized at attempt entry when their instance starts the attempt
(formals first, then Db `assertion_local_initializer`s in declaration order);
the inlined fallback: `assertions/locals.rs` module docs (copy-out is a
transition of its own). Property-level locals are forced into the antecedent graph and
inherited by the consequent (`PropertyLocalInit`); property-engine operands
reject them. Subroutine match items: `assertions/match_calls.rs` module docs
(`IrExprKind::MatchCall`, `DeferredAssertion` pre-fn). Sequence end points:
`assertions/endpoints.rs` module docs.
Sequence composition chooses its mechanism per site: one-cycle operands of
`and`/`intersect`/`throughout`/`within` stay one sampled `&&` atom; multi-cycle
operands become an `IrSequenceJoin`. Repeated non-Boolean sequences re-lower
their body per literal copy (an unbounded bound loops the last copy), within
`SEQUENCE_STATE_BUDGET`; never introduce a fixed unroll depth for `$`.
Assignments to locals inside join operands and differently clocked join
operands stay explicit rejections (16.14.1 makes cross-clock ones illegal). A
property that the predicate/sequence forms cannot represent
(`property_requires_engine`) lowers whole to an `IrProperty` node table
(`assertions/property.rs`): every node and sequence carries its own clock from
the program's clock list (leading clock first), single-tick operands are
sampled atoms on their clock, and the `strong`/`weak` default comes from the
assertion kind. A recursive re-entry (16.13.17), which Slang leaves
unexpanded, becomes `IrPropertyNode::Call` of the nearest enclosing expansion
of the same property whose actuals have equal structural keys and whose
flowing clock is the same (Annex F.7); the call site's actuals are read from
any expansion of the same site. Recursion through local variable formals is a
located limit. A non-signal
`disable iff` condition, and an inherited `default disable iff` (Db
`default_disable`, not for `expect`), become a hidden net driven by a
synthetic process and shared per instance and condition node. `expect` action
blocks lower inline in the calling process after `IrStmt::Expect`, branching on
`IrRuntimeQuery::ExpectOutcome`; other action blocks are spawned processes
that may contain timing controls. A concurrent assertion reached in procedural
code (`assertions/procedural.rs`, SV 16.15.6) registers as an ordinary
assertion with a dense `IrAssertion::procedural` index and lowers to
`IrStmt::QueueProceduralAssertion`; without a clock of its own it takes
`infer_procedure_clock` of `Codegen::procedure_body` before default clocking.
`const'` casts, procedure automatics and an inferred `edge` clock are located
rejections (no queued values are kept), and its operands are not implicit
sensitivities (S99-D2). Assertion-only `initial` and timing-free `always`
procedures keep the static path. Deferred
assertions retain issue-time values (packed, real, string, chandle/class handle
captures; a method receiver naming static storage is read at maturity with an
issue-time index, others are captured, S36-D3) and Reactive actions under the single-call
contract in process, subroutine, class-method and final contexts, rejecting
automatic/dynamic ref actuals, timing actions and container/event/virtual-
interface action arguments. `disable` of a labeled deferred assertion or of a
procedure's outermost block also emits `DeferredAssertionDisable` (SV 16.4.4).
A module-item deferred assertion uses always_comb sensitivity (SV 16.4.3).
Mailboxes preserve typed copy/identity semantics and delegate waits/cancellation to
runtime queues. Aggregate messages (records, unpacked arrays, queues, dynamic
arrays) go through a lexical native value of the message type
(`collection/native_values/messages.rs`) and the SIM-003/SIM-006 transfers; their
type identity is the static equivalence key `mailbox_message_key` (SV 6.22.2), and
the runtime only compares it. Destinations that are not whole variables receive into
a local and copy out after the retrieval with selectors frozen at the call (a `ref`
argument, Annex G.4); expression-form `try_*` calls that need this are hoisted only
before assignments, system-task calls and `if` conditions. `IrMailboxTarget::AtDelivery` destinations (AB-O3):
`mailbox_target_at_delivery` docs. Container kinds keep
distinct storage, key conversion, notification and lifetime rules; another container's admitted operation is not a fallback.
The frontend flattens `c[i][b]` into one select: split indices at the container
depth. A packed select of an element is one read/modify/write with indices and
RHS captured once, written back through the whole-element store. Mailbox element
types come from the DB's captured `T`, never the rendered parameter spelling.
