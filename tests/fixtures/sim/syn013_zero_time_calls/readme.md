# SYN-013 zero-time calls

`zero_time_calls.sv` exercises the selected Core closure for finite, delay-free
subroutines: named and dependent defaults, negative fixed-array bounds, nested
`const ref` forwarding, fixed aggregate copy-out, a local named-block disable,
and static versus automatic storage in separate generated instances.

`sim_syn013_zero_time_calls` also runs `static_array_nba.sv`,
`static_array_matrix.sv`, `static_array_delayed_nba.sv`,
`explicit_static_array_nba.sv`, `static_function_array_nba.sv` and
`legacy_static_array_nba.sv` through the
public CLI in both optimizer modes. They check declaration initialization,
post-return publication, selected rows and slices, repeated calls, independent
module instances, delayed NBA order, function-scheduled NBAs and
element-sensitive wakeups. The legacy
fixture runs in both Verilog-2001 and SystemVerilog-2009; SV fixed-array value
and explicit-lifetime forms run in 2009. The two automatic-array/formal fixtures
are separate negative controls under IEEE 1800-2009 §10.4.2.

The source uses IEEE 1800-2009 §§6.21–6.22 and 13.3–13.5. The local section
index is [`docs/specification/spec-reference-sv.md`](../../../../docs/specification/spec-reference-sv.md).

This is bounded execution evidence. Timed recursion, event/process formals,
resizable/native aggregate ABIs, and illegal reference/NBA combinations remain
outside this Core task.
