# Nonblocking assignments under an event control

The task module (`sim_feature_completion/ki_event_updates.rs`) runs the fixtures
through the public CLI in both HDL optimizer modes. Order and endings under
other controls are decision S09-D11. Clause lines are from
`SystemVerilog-1800-2009.txt`.

## Clocking

`clocking.sv`: 14.13 L19922-19923: "This event shall be triggered in the Observed
region." 9.4.5 L12123: "An intra-assignment delay or event control shall delay
the assignment of the new value to the left-hand side". The control queues the
update from the active events that follow; queued in the Observed region
itself it was an error at the event. Local, interface and virtual interface
clocking blocks, `repeat`, `iff`, packed, real, string, handle and element
targets. An industry-standard simulator does not support such a control.

## Captured loop variables

`loop_variable.sv`: the storage of a loop variable that forked processes
captured ends with the loop, in the turn of its last write, whose check at the
end of the turn read the freed storage. Only the sanitizer lane shows the defect.

## Event variables

`event_objects.sv`: 15.5.5.1 L20888-20890: "When events are merged, the
assignment only affects the execution of subsequent event control or wait
operations. If a process is blocked waiting for event1 when another event is
assigned to event1, the currently waiting process shall never unblock." An
assignment of a string, handle, element or record and a `->>` whose control a
process waits for hold the event of their control from where they are issued,
and the `->>` that of its target; so does a control that lists an event beside
an expression its process evaluates. Events of variables, array, queue and
associative elements, class properties, virtual interfaces and formals. An
industry-standard simulator refuses the string and record targets and stops at
such a list with a null event report; it agrees on the rest.

`null_event.sv`: decision S14-D8 for the same controls. The run ends at the
statement; `repeat (0)` does not wait and is no access.

## Real arrays and `.triggered`

`real_arrays.sv`: event-controlled and delayed nonblocking writes of a whole
real array and of its elements were refused ("cannot resolve assignment
target"), and an event control whose `iff` qualifier reads `ev.triggered`
was refused as a call without an effect summary. 9.4.5 L12123-12125: "An
intra-assignment delay or event control shall delay the assignment of the new
value to the left-hand side, but the right-hand expression shall be evaluated
before the delay, instead of after the delay." 15.5.3 L20735-20736: "The
triggered event property evaluates to true if the given event has been
triggered in the current time step and false otherwise."
