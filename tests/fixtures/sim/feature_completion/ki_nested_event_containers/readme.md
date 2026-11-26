# Queues and arrays of event queues

A queue, dynamic or associative array whose elements are event queues
(`event w[$][$]`, `event am[int][$]`, `event dd[][$]`), and a fixed array of
event queues (`event d[2][$]`), were refused wherever they were declared
("event container element ... requires an event handle or null"): an event
container held only event handles. Such an array now nests like a container
of any other element type, and its elements are event handles.

The task module (`sim_feature_completion/ki_nested_event_containers.rs`)
runs the fixtures through the public CLI in both HDL optimizer modes and on
every value backend. Expected output was checked against an independent
industry-standard simulator, which gives every line. Clause lines are from
`SystemVerilog-1800-2009.txt`.

- 6.17 L5946-5947: "The event data type provides a handle to a
  synchronization object. The object referenced by an event variable can be
  explicitly triggered and waited for."
- 7.10 L8975 and L8979-8980: "A queue is a variable-size, ordered collection
  of homogeneous elements." ... "Thus, like arrays, queues can be manipulated
  using the indexing, concatenation, slicing operator syntax, and equality
  operators."

`nested.sv`: such arrays in a module, a class property and an automatic task
variable: `push_back` of a concatenation, an element `push_back`, row and
element writes (also `null`), `@` on an element, `->` of an element,
`.triggered`, `size()` of a row, equality of elements, `void'(pop_front())`
of a row, a two-index `foreach` and `delete`.

`blocks.sv`: the same arrays declared in a named procedural block (a queue of
event queues and a fixed array of them) and in a static task.
