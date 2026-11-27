# FIN-002: the remaining rows of the failure matrix (step 4)

Run by [`tests/sim_feature_completion/fin_002f.rs`](../../../../sim_feature_completion/fin_002f.rs),
whose table names the fixtures of each row. The other rows are in
[`fin_002b`](../fin_002b/readme.md) and [`fin_002d`](../fin_002d/readme.md), the
language server's in `tests/lsp_stdio/storms.rs`, SDF values in
[`adv_012`](../adv_012) and the `adversarial_inputs` corpora.

A table (`.out`) holds one section per run of one model: the plusargs after
`###`, the standard output, and after `===` the exit status and the ids of the
warnings and errors. Both optimizer modes run every section. Unless a paragraph
below says otherwise a section is the industry-standard behaviour (the same
source and plusargs). Runs whose result differs from it and that the text does
not decide are left out; they are listed here.

## Cancellation

`cancel_calls`, `cancel_expressions`, `cancel_final`, `cancel_sequences` and
`cancel_regions` are the industry-standard behaviour. `cancel_final`, `cancel_regions`
and `cancel_dpi` have several final procedures: they run in the order of
decision S09-D12 (9.2.3 leaves the order to the implementation). Left out: `disable` of a
block, such as the one of a `final` procedure, from a function it calls or from
the function of a coverpoint expression:

> 12401: not be used to disable functions. In cases where a disable statement within a function disables a block or a
> 12402: task that called the function, the behavior is undefined. Disabling an automatic task or a block inside an

`kill()` of the sampling process in a coverpoint expression, where `self()` may
be null; `kill()` in `post_randomize` of a call whose result is assigned (the
assignment is or is not made):

> 12633: other condition, such as an event, wait expression, or a delay, then the process shall be terminated at some
> 12634: unspecified time in the current time step.

and the pass action of an `assert property` (decision S38-D11 runs it for vacuous
successes).

The `act=stop` sections, here and in `cancel_dpi`, are llg's own: a batch run
goes on after `$stop` ([stop/resume](../../../../../docs/sim_features.md)), so each is the `act=none`
run. In a final procedure there is nothing to go on with and `SIM-011` says so.

`cancel_dpi`, `cancel_dpi_calls`: an exported task that a `disable` or `kill()`
ends returns 1 to its C caller, which then is in the disabled state:

> 56270: a) When an exported task returns due to a disable, it shall return a value of 1. Otherwise, it shall
> 56271: return 0.

> 56264: immediately after the return of a call to an exported subroutine. An important aspect of the protocol is that
> 56265: disabled import tasks and functions shall programmatically acknowledge that they have been disabled.

A `kill()` of the process in an exported *function* leaves no disabled state
(9.7 has no counterpart of 35.9 for it) and the caller ends when the import
returns. `$finish` and `$fatal` in an exported subroutine never return to the C
code (decision S41-D9). `disable fork` in an exported task ends the children of
the process and nothing else:

> 12548: The disable fork statement terminates all active descendants (subprocesses) of the calling process.

A simulator may leave the children of the process that called the import alone,
although the same task called without C in between ends them.

`cancel_vpi_regions`, `cancel_vpi_statements`: the C code goes on after
`vpi_control` and the run ends when its routine returns:

> 61988: vpiFinish                   Causes the $finish built-in SystemVerilog system task to be executed upon
> 61989: return of the application routine.

A callback that a statement runs is inside that statement, so the statement after
it does not run. Where some simulators refuse `cbDisable`, `cbAtStartOfSimTime`
or `cbNBASynch` without a pending assignment, or run `cbRelease`, `cbError` and
`cbStartOfThread` later, the section holds what 38.36 says. The value change
callback of an assignment runs at the next check (decision T1-D11), here the
`$display`, which then does nothing.

## Stale handles

`null_handles` is the industry-standard behaviour, except that `.triggered` of a
null event is false (contexts 75, 133: 15.5.3 L20736-20737, decision S14-D9) and
`get_coverage()` is a static method (context 81: 19.8 L33569-33571, S44-D11). Every
wait on a null event ends the run (S14-D8); the state methods of a null class
handle go on with the calling process (S28-D10). The null receiver of context 128
therefore changes nothing.

`vpi_stale`: a handle of an element selects it at each access (decision A19-D23),
so it fails while there is no such element and reads the new one afterwards; the
local of an ended frame fails; a class object that a handle names stays. A pending
event's handle is kept on purpose past the end of the run (the event is no longer
in the queue, the handle still is the application's):

> 64210: Calling vpi_release_handle() on the handle shall free the handle, but shall not affect the event.

`vpi_handles` makes chosen calls on handles in every state; nothing but the end of
the run is expected. Both are meant for the sanitizer lane
([tests readme](../../../../readme.md)).

`foreign_chandle`: the value is the C pointer, so a chandle whose object was freed
still compares equal and is passed back as it is; nothing reads through it.

> 5593: —    Chandles shall not be assigned to variables of any other type.

so a `$cast` to a class handle fails.

## Simultaneous events

`event_storm` runs N processes of each kind, 24 and 200. Active events are taken
"and processed in any order" (3466); llg and the industry-standard behaviour run a
process to its next time control, which 3467-3470 allow. For every order:

| Line | Accepted |
| --- | --- |
| `race r ran n` | the last writer: r in 0..N-1; every process ran: n = N |
| `nonblocking a b changes c` | the last process to schedule wins both variables (10.4.2 keeps the order of one process): a in 0..N-1, b = 1000 + a; the always sees at least the first change and no more than one per update: c in 1..N |
| `events e l t a` | all waited before: e = N; a wait of the same step may come late: l in 0..N; the triggered state lasts for the step (15.5.3): t = N; `->>` triggers after every wait of the step: a = N |
| `gates g parity p changes c` | the settled value: g = 7f, p = 1; eight bits that rise one after the other and one that falls again: an odd c from 1 to 9 |
| `zero t e sample s` | every loop ran: t = 5N, e = N; the sampler runs among the second turns: s in N..2N |
| `mailbox got g distinct d left l taken t polled p` | no message twice or lost: g = d = t + p, g + l = 3N or the mailbox is full (l = 2); a getter waits only at an empty mailbox: l = 0 or t = N |
| `semaphore peak p entered e all back b` | three keys: p in 1..3; e = N, b = 1 |
| `disable done d finished f killed k` | a child ended or was ended: d = f, f + k = N |
| `join any a all n` | the other children of the step ran before or after: a in 0..N; n = N |
| `strobe s` | nonblocking updates follow the `#0` writes: 1000 |

## Wrong types

`wrong_cast` is the industry-standard behaviour:

> 7437: It is important to note that $cast performs a run-time check. No type checking is done by the compiler,
> 7438: except to check that the destination variable and source expression are singulars.

`wrong_plusargs` is the industry-standard behaviour. Left out: texts where some
simulators extend an x or z digit to the left (37585 pads with zeros), take a sign
with `%h`, `%o` and `%b`, or wrap a decimal text at 32 bits before they convert it
to a real variable.

`wrong_readmem` is the industry-standard behaviour but for contexts 6, 19 and 23,
which follow the text:

> 37299: Both uppercase and lowercase digits are allowed in the number. No white space is allowed between the @
> 37300: and the number.

> 37316: data file shall be within the address range specified by the system task arguments; otherwise, an error mes-
> 37317: sage is issued, and the load operation is terminated.

and a `z` digit is 0 in a two-state word (21.4.2).

`wrong_mailbox`: rows are messages, columns variables; `+` only where the types are
equivalent (6.22.2: the same width, signedness and state count; enumerations and
unpacked structures only themselves).

> 20582: method returns 0. If the type of the message variable is not equivalent to the type of the message in the
> 20583: mailbox, the method returns a negative integer.

`wrong_vpi_formats`: one letter per format in the order of `formats[]` (binary,
octal, decimal and hexadecimal string, scalar, integer, real, string, vector,
strength, time, object type, suppress, 99), `.` served and `E` an error; a refused
write changes nothing. Objects without a value (events, whole arrays, structures,
class handles, scopes) have no readable format; a real variable has no scalar,
vector and strength, and by 64217 no string; a vector takes no strength
(64218-64219); a string takes a string or an integer, a chandle the formats of
decision A19-D21 (A19-D25); a named event takes a put of any format (A19-D8) and no
delay mode; parameters are not written; a dynamic array element takes no delay and
no force (A19-D23).

`reject_dpi_types`, `reject_object_types`: every line with a `// wrong:` mark is
refused with that id and no other line is (6.22, 8.12, 15.4.9, 19.8, 25.9, 35.5.4-35.5.6).
