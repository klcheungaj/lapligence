# Declaration initializers of arrays of events

An array of events with a declaration initializer stopped the import
("named-event arrays cannot have declaration initializers", ELAB-002, without
a source position). The initializer is now kept and lowered as assignments of
the elements, as that of a scalar event is.

The task module (`sim_feature_completion/ki_event_initializers.rs`) runs the
fixture through the public CLI in both HDL optimizer modes and on every value
backend. Expected output was checked against an independent industry-standard
simulator, which gives every line. Clause lines are from
`SystemVerilog-1800-2009.txt`.

- 6.17 L5961-5966: "An event variable can be assigned or compared to another
  event variable or assigned the special value null. When assigned another
  event variable, both event variables refer to the same synchronization
  object. [...] If an initial value is not specified in the declaration of an
  event variable, then the variable is initialized to a new synchronization
  object."
- 7.4.2 L8060-8062 and 10.9 (assignment patterns) give the element order: the
  left index first.

`arrays.sv`: a two-dimensional static array with nested, positional and
`default:` patterns, a keyed pattern, a whole array copied from another, a
queue, and fixed arrays that are instance and static class properties; waits on
initialized elements.

`blocks.sv`: a queue and a dynamic array of events declared in a procedural
block were refused ("multidimensional event array ... in subroutine storage",
without a position).
