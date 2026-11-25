# Nonblocking assignments under an event control

The task module (`sim_feature_completion/ki_event_updates.rs`) runs the fixture
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
