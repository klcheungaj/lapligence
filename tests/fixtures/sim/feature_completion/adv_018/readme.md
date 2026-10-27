# ADV-018: DPI-C call chains whose exported tasks consume time

These fixtures cover context imported tasks whose C code calls exported tasks
that wait (`#`, `@`, `fork ... join`), with several chains suspended at once,
nested C -> SV -> C -> SV chains, scope moves, the disable protocol of a
suspended chain, a kill of a process whose chain is suspended, and `$finish`
with a chain still suspended. Each positive `<name>.sv` has a portable C11
companion `<name>.c`; the runtime negatives share `errors.c`. The test
[`adv_018.rs`](../../../../sim_feature_completion/adv_018.rs) builds the
companion into a shared library (a DLL linked against the design's import
library, `llg --dpi-import-lib`, on Windows) and passes it with `--dpi-lib`;
import definitions carry `DPI_DLLESPEC` as in
[sim_040](../sim_040/readme.md). `crossing` also runs with the companion built
into a static archive, which needs no import library. Positives run on the
legacy, compact/portable and compact/GMP value backends in both optimizer
modes. Companions build with `-std=c11 -Wall -Wextra -Werror` (MSVC `/W4 /WX`
for the archive). The mechanism (foreign call contexts) is described in
[`src/sim/rt/foreign_contexts/readme.md`](../../../../../src/sim/rt/foreign_contexts/readme.md).

Line numbers refer to `SystemVerilog-1800-2009.txt` (a `pdftotext -layout`
extraction of IEEE 1800-2009).

## Clauses

35.5.1.5 (L55696-55701):

> A call to an imported task can result in the suspension of the currently
> executing thread. This occurs when an imported task calls an exported task,
> and the exported task executes a delay control, event control, or wait
> statement. Thus it is possible for an imported task's C code to be
> simultaneously active in multiple execution threads. Standard reentrancy
> considerations must be made by the C programmer.

35.5.3 (L55801-55807):

> — When a SystemVerilog subroutine calls a DPI context import, a context for
> the import call chain is created that is equal to the instantiated scope of
> the import declaration.
> — When a routine in an import call chain invokes svSetScope with a legal
> argument, the call chain's context is set to the indicated scope.
> — When a call from an import call chain to an exported SystemVerilog
> subroutine finishes and returns to the chain, the call chain's context is set
> equal to its value when the call to the export was made.

35.8 (L56241-56245):

> It is never legal to call an exported task from within an imported function.
> This semantics is identical to native SystemVerilog semantics, in which it is
> illegal for a function to perform a task enable.
>
> It is legal for an imported task to call an exported task only if the
> imported task is declared with the context property.

35.9 (L56262-56264):

> An imported subroutine is said to be in the disabled state when a disable
> statement somewhere in the design targets either it or a parent for
> disabling. An imported subroutine can only enter the disabled state
> immediately after the return of a call to an exported subroutine.

35.9 (L56270-56277):

> a) When an exported task returns due to a disable, it shall return a value of
> 1. Otherwise, it shall return 0.
> b) When an imported task returns due to a disable, it shall return a value of
> 1. Otherwise, it shall return 0.
> c) Before an imported function returns due to a disable, it shall call the
> API function svAckDisabledState().
> d) Once an imported subroutine enters the disabled state, it is illegal for
> the current function call to make any further calls to exported subroutines.

35.9 (L56282-56284):

> Item a) is guaranteed by SystemVerilog simulators. In addition, simulators
> shall implement checks to verify that item b), item c), and item d) are
> correctly followed by imported tasks and functions. If any protocol item is
> not correctly followed, a fatal simulation error is issued.

35.9 (L56309-56311):

> If an exported task itself is the target of a disable, its parent imported
> task is not considered to be in the disabled state when the exported task
> returns. In such cases, the exported task shall return value 0, and calls to
> svIsDisabledState() shall return 0 as well.

35.9 (L56314-56318):

> When a DPI imported subroutine returns due to a disable, the values of its
> output and inout parameters are undefined. [...] However, SystemVerilog
> simulators are not obligated to propagate any such values to the calling
> SystemVerilog code if a disable is in effect.

9.7 (L12631-12632):

> The kill() function terminates the given process and all its subprocesses,
> that is, processes spawned using fork statements by the process being
> killed.

20.2 (L34119):

> The $finish system task causes the simulator to exit and pass control back
> to the host operating system.

## Expected outputs

`crossing` (A01; `timescale 1ns/1ns): `sv_wait` is automatic, so each call
has its own `id`. Chain 1 enters at 0 and waits 10; chain 2 enters at 5 and
waits 20. Chain 1 resumes at 10 and its C code finishes while chain 2's C
frames are still suspended below it in call order (35.5.1.5: the C code is
active in two threads at once), so it prints `c_chain 1 back` at 10 and
chain 2 follows at 25. Chains 4 (enter 40, wait 30) and 3 (enter 50, wait 5)
resume last in, first out at 55 and 70. Every C frame finds its 256-byte
pattern and its `volatile` local unchanged (`intact=1`), and every exported
task returns 0: nothing was disabled (35.9 a).

`nested` (A02): `u1` calls `c_outer` at 1, `u2` at 2 (`#(ID)`). Each
`sv_step` waits for `fork #2; #3; join` (3 units, until 4 and 5) and calls
`c_inner`, which moves the chain to the other instance with `svSetScope`
(35.5.3, L55803-55804) and calls that instance's `sv_inner_wait`, an event
wait: the `u1` chain waits on `u2.go` from 4, the `u2` chain on `u1.go` from
5. Before waiting, `c_inner` calls the zero-time exported function
`sv_count` in the scope it moved to: that instance's `count` becomes 1, so it
returns `1 * 100 + ID`, 102 for the `u1` chain (in `u2`) and 101 for the `u2`
chain. `tb` triggers `u1.go` at 20 and `u2.go` at 30, so the chain started second
returns first. `got = depth * 10 + ID` uses the `ID` of the instance whose
export ran: 11 for the `u2` chain (`u1`'s export) and 12 for the `u1` chain.
After each export returns, `svGetScope()` is the scope set before the call
(L55805-55807, `scope_is_other=1`), and after `c_inner` restores the previous
scope `c_outer` finds its own scope again (`same_scope=1`). `sv_step` reads
its string input after the suspensions and returns `r + 1` (12 and 13);
`c_outer` returns `got * 2` (24 and 26).

`disables` (A02; 35.9):

1. `blk1` is disabled at 10 while `sv_hold(1)` waits: the exported task
   returns 1 (item a), `svIsDisabledState()` is 1, the C code returns 1
   (item b) and its output is not copied back (L56314-56318, decision
   S41-D5): `o1` stays 5.
2. `disable sv_hold` at 30 targets the exported task itself: it returns 0,
   `svIsDisabledState()` is 0 (L56309-56311) and the import completes, so
   `o2` is 42.
3. `p3.kill()` at 50 kills the process whose chain waits since 40
   (decision A18-D1): the chain is unwound at the kill, the exported task
   returns 1 with `svIsDisabledState()` 1, and the C code returns 1. Then
   the killer prints `o3=7` (nothing copied back) and `KILLED`.
4. Chain 4 waits from 60 until 160, but `$finish` at 70 ends the simulation
   (20.2): its C code never runs again (decision A18-D2), the final block
   prints `o4=8` and the run ends with status 0.

## Negative cases

`neg_function_wait`: the imported function `c_func` calls the exported task
`sv_wait_e`, which can consume time. 35.8 (L56241-56242) makes the call
illegal; llg reports a fatal error at the call, whether or not the call
would wait (decision A18-D3). The export returns 0 to the C code, which
prints `c_func status=0` and returns; nothing runs after the import.

`neg_missing_ack`: `blk` is disabled at 10 while `sv_wait_e` waits; the
export returns 1, but `c_no_ack` returns 0, which breaks item b): a fatal
error after `c_no_ack status=1 returns 0`.

`neg_kill_call_again`: the process whose chain waits is killed at 10; the
unwound C code calls `sv_wait_e` again after the export returned 1, which
breaks item d): a fatal error, and that C code never runs on (`c_again not
reached` is not printed); the run ends instead of hanging.

## Other implementations

Not yet compared with another simulator (reference check pending). The
decisions A18-D1 to A18-D3 have portable cases in
`tests/fixtures/sim/lrm_decisions/`.
