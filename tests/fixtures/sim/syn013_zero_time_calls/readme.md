# SYN-013 zero-time calls

`zero_time_calls.sv` exercises the selected Core closure for finite, delay-free
subroutines: named and dependent defaults, negative fixed-array bounds, nested
`const ref` forwarding, fixed aggregate copy-out, a local named-block disable,
and static versus automatic storage in separate generated instances.

The source uses IEEE 1800-2009 §§6.21–6.22 and 13.3–13.5. The local section
index is [`docs/specification/spec-reference-sv.md`](../../../../docs/specification/spec-reference-sv.md).

This is bounded execution evidence. Timed recursion, event/process formals,
resizable/native aggregate ABIs, and illegal reference/NBA combinations remain
outside this Core task.
