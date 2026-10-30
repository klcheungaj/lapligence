# Runtime: stop, synchronization and assertions

`$finish`/`$stop`, `llg_rt_resume` and `LLG_STOP_POLICY`: `llg_rt.h` comments. Do
not run finals while
suspended; generated CLI exit policy cleans up explicitly. Validate verbosity 0/1/2
without changing semantics. Normal completion/severity termination runs final
handling exactly once and preserves severity counters. A run-ending simulator
error that skips final procedures calls `llg_rt_skip_finals` before its fatal
report (S14-D6, S16-D3, S29-D1, S31-D4); the flag lives outside `g` and resets
at init, and its callers' generated statements carry a Terminate effect so the
process takes its exit check.

Semaphore service order (AB-N4) and suspended waiters (AB-N1):
`scheduler/wait_queues.c` and `process_control.c` comments.
Mailboxes retain bounded/unbounded FIFO/peek rules and nominal typed identity
(including null handles), packed width/sign/state and real precision.
Mismatched try calls return -1 without assignment/consumption; blocking mismatches
fail. Blocked-get service, delivery targets, published handles, killed waiters,
`LLG_MAILBOX_VALUE` ownership and `llg_mailbox_trace` (AB-O3, SIM-018): the
mailbox block of `llg_rt.h` and `scheduler/mailboxes.c`.

Clocking input samples complete before the Observed block event. Output captures
publish Re-NBA with constant skew; off-event drives wait for the next event and
`##N` counts events. `scheduler/dependencies.c` owns exact-descriptor edge history:
clocking synchronous drives and `##0` can query after publication, including after
registration in the same slot. Therefore publication records even automatic
locals without an existing waiter; subscriber absence does not prove an entry
unusable. Generated clocking-source emission currently rejects automatic bindings,
but native runtime callers can pass any live packed owner to the borrowed-source
APIs. Globally filtering that storage class would change accepted runtime behavior.
Recording is the default. A generated model whose IR has no clocking drive, `##`
cycle wait or assertion clock pulse (`IrModel::clocking_history`, set by lowering
and checked by validation) calls `llg_rt_skip_clocking_history`; publication then
updates only the registered sequence clocks' tick entries, so ordinary designs
pay no per-change entry allocation or time-advance free.
Wait/sampling split and clockvar publication: `dependencies.c` and
`sampling.c` comments. A real or shortreal input bound to an expression samples its 64-bit
`$realtobits` image signal; the `_real` variants decode it into the `double`
sample.
`llg_sampled_register_value` keeps only a signal's Preponed value (procedural
`$sampled`, every concurrent-assertion read) in a separate list, so it adds no
history and no per-write lookup. `llg_sampled_register` adds Observed clocking
copies; `llg_sampled_register_history` keeps the slots the deepest declared input
skew can select (the newest at or before `now - ticks` and everything newer),
promoting an earlier value-only entry. Sampled-value clocks and domains use
dense model-assigned identities (O(1) lookup). An edge clock is matched at the
signal write, its gate read with current values; an event clock ticks only
through `llg_sampled_clock_tick` from a generated waiting process. Each domain
keeps a ring of one sample per ticked time step (a repeated tick overwrites
the step), growing up to its registered `$past` depth plus the current step and
then reused in place; status functions compare a step-cached Preponed
evaluation with the latest strictly earlier tick. Never retain history for an
undeclared reader: memory must not grow with simulated time.

Slot history retains any/positive/negative occurrence independently, so a later
opposite edge cannot erase an earlier match. Time advance drops all entries except
registered sequence clocks, clears their occurrence flags and compacts the pointer
index. Sequence registration marks the root and explicit transition clocks before
execution; only those consumers need cumulative saturating directional counts for
same-clock delays across slots. Disabled assertions may later resume, so their
clock counts last until cleanup, independent of attempt lifetime. No count resets
at time advance. Cross-clock replay history in the assertion itself is separate.
Packed scope final release forgets every descriptor before freeing it; lexical
exit alone cannot forget a cell retained by queued writes. External packed owners
must call `llg_clocking_forget_signal` before freeing/reusing a written descriptor,
and must separately satisfy wait/sample/assertion source lifetimes. Model storage
survives runtime cleanup. The history list owns nodes; its hash index borrows them,
uses exact pointer equality, and must be updated on removal/reset/teardown. This
private layout/additive API leaves process ABI 3 and value ABI 4 unchanged;
embedded runtime-content hashing invalidates earlier cache archives.

Deferred immediate assertions keep issue-time sampled values; Reactive refs
resolve at action time. The report queue model is the `deferred_assertions.c`
header (per-process chains through `llg_proc_t::deferred_last`); resuming from an event control or wait
statement (`deferred_flush`, set by `wake_proc`, applied before `llg_co_run`),
an outermost-scope disable and a specific-assertion disable flush pending
reports; one Reactive callback runs the matured array. OFF prevents new checks but does not cancel pending
actions; KILL flushes only unmatured reports. `$finish` and the end of the run
drop every queued report, pending or matured (S36-D6;
`deferred_assertions_finish`, which still runs queued match-item calls); a
final procedure's own reports execute when it returns (S36-D4). These fields fit existing `llg_proc_t` padding.
Procedural concurrent assertions share that queue (`LLG_DEFERRED_PROCEDURAL`
comment): `llg_assertion_queue(index)` appends a value-free
`LLG_DEFERRED_PROCEDURAL` record (none while the assertion is off) for the
dense index registered by `llg_assertion_procedural`, and
`g.procedural_pending_live` counts the live ones so the deferred paths stay
untouched without them. `run_concurrent_assertions` first matures them
(`procedural_assertions_mature`): each instance adds one to the assertion's
`procedural_matured`, or begins at once when the leading clock ticked earlier
in the time step (the saved `procedural_event`/`procedural_cycle`, decision
S99-D4). Each root clock event takes all matured instances as attempts;
matured instances begin even while the assertion is off; KILL drops pending
and matured instances (S99-D3). Procedural assertions are never
single-attempt, end points or `expect`.
`llg_sequence_match_call` appends a sequence match-item call (SV 16.11) with
its owned argument frame directly to the matured array (kind
`LLG_DEFERRED_MATCH_CALL`): it runs in the Reactive region in queue order with
no assertion result, counter or control.

Assertion control rules (`assertion_control.c`) carry a field: checking, pass,
vacuous pass or fail. The newest matching rule wins; the six SV 20.12 tasks
set the action flags of matching concurrent assertions and add rules that
immediate, deferred (read when the report executes) and `expect` actions
consult through `llg_immediate_assertion_controls` and
`llg_deferred_assertion_enabled`. `assertion_result` records every result and
counter before it gates the action, so controls never change statistics; a
suppressed default failure still counts (`llg_assertion_failure_quiet`).
Sequence end points (`llg_assertion_endpoint`) are hidden covers skipped by
controls and counters; match visibility (S39-D5, `also_next`):
`sequence_endpoint_matched` comment in `concurrent_assertions.c`.

Sequence tokens own a transition, local snapshot and retained first-match scope
chain. Each scope belongs to an invocation; completion cannot discard outer
siblings/suffixes. Preserve all earliest tied endpoints. Empty-word alternatives
use `admits_empty`/normalized concatenation, not current-tick epsilon; negative
repetition guards use four-state truth. Consequents inherit locals by declaration
ID, initializing only private cells. Common discard paths release snapshots on
abort/shutdown. Cross-clock ##0 uses nearest at-or-after physical time, ##1 strictly
later time; retain current-slot edge history for all consumers. Callback order is
only a replay guard, never elapsed cross-clock time. Explicit action choice must
not erase assertion failure accounting.

An implication attempt (`R |-> P`) is one `llg_assertion_eval_t` shared by its
antecedent attempt and every consequent a match of `R` starts; it reports one
result (IEEE 1800-2009 16.13.6): failure when a consequent fails, success when
`R` is exhausted and every consequent succeeded. Decided attempts discard their
remaining threads; abort/disable paths report an attempt once. `cover sequence`
registers as `LLG_ASSERTION_COVER_SEQUENCE` and reports every nonempty match
(16.15.3). An armed `expect` starts exactly one attempt (16.18).
`llg_assertion_gate_clock` attaches a leading clock's `iff` gate (current values
at the clock write, 9.4.2.3); a gated-off edge appends no clock event, and the
assertion counts gated ticks itself so `##n` delays skip gated-off edges.

General property operators (`properties.c`, `llg_assertion_register_property`)
run per attempt as a tree of instances over the static `llg_property_node_t`
table (operands precede users; the last node is the root). Each instance
resolves once to true/false with its 16.15.8 nonvacuity; a parent may decide
while an operand is pending, which then counts as vacuous (decision S38-D3).
Each result also carries its cover vacuity (S38-D10): a cover counts every
success as a hit except the vacuous success of an implication without an
antecedent match or of an else-less `if`, passed on through `not`,
`nexttime`, an unfired abort or implication consequents; assert and assume
keep the 16.15.8 vacuity.
A multiclocked program lists its clocks (`llg_property_clock_t`, the leading
clock first) and each node and sequence names its own; an instance starts at a
launch point (clock, time, tick) and advances on the first tick of its own
clock at or strictly after it, so coincident ticks of different clocks are one
time step (16.14). Both branches of an `if`-`else` start at the tick that
reads its condition (A13-D3). Single-clock programs have no clock list and run
on the assertion's clock. Sequence operands reuse the sequence engine on their clock
with their own zero-delay ranks. `LLG_PROPERTY_CALL` (a recursive property
instance, 16.13.17) starts a fresh instance of its target node.
`llg_assertion_clock_pulse` writes a planned clock's hidden signal 1 then 0 at
most once per time step. Asynchronous `accept_on`/`reject_on` are checked once per
time step from the write hook with sampled values; synchronous ones and every
abort are checked first on each tick. At the end of simulation
(`run_end_of_simulation_assertions`, after deferred reports) a pending attempt
fails (fail action in an isolated Reactive pass) only if it waits on an
`s_eventually` obligation, begun or due to begin through a started operator
(`property_end_fails`, a per-program eventuality table built at the end);
every other pending attempt, strong obligations included, ends without a
result (S38-D1, A13-D7);
`cover` and `expect` are not settled. Instances count against
`LLG_SEQUENCE_THREAD_LIMIT` and are recycled through `g.property_inst_pool`.

Global clocking future sampled value functions (16.9.4,
`future_assertions.c`, registered by `llg_assertion_future` after the
assertion): such an assertion queues each Observed pass's clock events with
the operands its generated capture callback stores, the attempts a
leading-clock event begins (enable state, single attempt and procedural
maturity read then), the step's clock history and its action controls, and
evaluates the entry through the ordinary engines in the Observed region of
the first tick of its global sampled clock (`tick_time1`) strictly after the
entry's step. Predicates read the captured operands through
`llg_assertion_captured` (fatal outside a replay) and the future arguments as
current sampled values, so results and actions belong to that global tick
while every interval rule is that of the event. `end_assertion_attempts`
(disable iff, `$assertkill`) appends a marker instead while entries are
queued, so a discard reaches exactly the attempts running at its point;
consecutive markers coalesce. An attempt that starts while `disable iff`
holds is queued as a disabled start, so its callbacks run at the next global
tick with the results before it (A17-D7). A procedural instance maturing after the step's
leading-clock event was queued adds to that entry. Entries count against
`LLG_SEQUENCE_THREAD_LIMIT` and are recycled per assertion with their operand
owners; the end of the run drops the queue (A14-D3) and cleanup frees it.
Assertion coverage counts an attempt and its result when the entry is
evaluated. `expect`, end points and asynchronous aborts are rejected by
codegen and by the registration.
An `expect` keeps its outcome (`llg_assertion_expect_outcome`: 0 none, 1 pass,
2 fail) for the resumed caller, which runs the action block itself; an else arm
(`has_fail_action`) suppresses the default failure report.
`llg_assertion_single_attempt` limits an `initial`-procedure assertion to one
attempt (16.15.6).

Sequence joins (`and`/`intersect`, and `throughout`/`within` reduced to
`intersect`) are static graph tables: the enter edge forks one thread per
operand with its own side frame on the scope chain; exit edges pair endpoints
(intersect: same tick; and: the later tick, an empty-admitting operand counts as
one match at the fork) and continue with the arriving thread's multiplicity
times its partner count, each pair counted by its later arrival. A side's
liveness is its frame refcount; after every step prune threads whose join can no
longer pair, to a fixed point, so impossible obligations fail instead of pending.
Match multiplicity (IEEE 1800-2009 16.9.5-16.9.8): a token and an endpoint
carry `mult`, the number of distinct paths they stand for; equal tokens merge
by adding it. Same-step work runs in the per-graph zero-delay topological
rank computed at registration, so a state expands only once all its
same-step paths have merged. Ways of matching the empty word count once.
Match items and `cover sequence` pass statements run once per path, which is
enumerated only there and only within `LLG_SEQUENCE_THREAD_LIMIT`; an
exhausted budget or a count above 2^64 - 1 is a reported error.
Tokens, frames, joins, endpoints, attempts and assertion clock events are
recycled through `g` free lists released by `llg_rt_cleanup`; keep steady-state
stepping allocation-free. Live tokens plus attempts count against
`LLG_SEQUENCE_THREAD_LIMIT`; exhaustion is a reported failure, never a dropped
attempt.

Free checker variables (`free_variables.c`, IEEE 1800-2009 17.7.2):
`llg_checker_free_set` registers one set per checker instance (solve
callback, assume identities resolved by one sorted pass on the first solve,
a stream derived like a static process of the instance). The top of every
Observed set calls `solve_free_variables` only when a set exists: a set
with assumes solves when a leading clock event of one of them waits for
that pass, a set without assumes once per time step. While the callback
runs, `llg_checker_assume_flag` reports each assume's event and pending
`|=>` consequent, randomize draws from the set's stream and an unsolved
problem returns 0 without a report.
