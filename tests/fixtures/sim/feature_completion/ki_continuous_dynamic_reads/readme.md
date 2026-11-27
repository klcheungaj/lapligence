# KI continuous dynamic reads

A reference to an element or member of a dynamic variable in a continuous
context is an illegal design: a located frontend error
(`SLANG-DynamicNotProcedural`, exit 1, nothing printed) whose text names the
clauses. The suite is
`tests/sim_feature_completion/ki_continuous_dynamic_reads.rs`; every fixture
has top `tb` and a `timescale 1ns/1ps. The industry-standard simulator
rejects these forms too, except an input string port connected to an
element of a queue or associative array, which it runs; the text makes that
illegal as well, and llg follows the text (decision A19-D95).

## Clauses

IEEE 1800-2009, `SystemVerilog-1800-2009.txt`:

- 6.21 L7007-7010: "Automatic variables and members or elements of dynamic
  variables—class properties and dynamically sized variables—shall not be
  written with nonblocking, continuous, or procedural continuous
  assignments. References to automatic variables and elements or members of
  dynamic variables shall be limited to procedural blocks."
- 25.9 L44930-44932: "Once a virtual interface has been initialized, all
  the components of the underlying interface instance are directly
  available to the virtual interface via the dot notation. These components
  can only be used in procedural statements; they cannot be used in
  continuous assignments or sensitivity lists."
- 23.3.3.2 L41468-41469: "An input port can be connected to any expression
  of a compatible data type. A continuous assignment shall be implied when a
  variable is connected to an input port declaration."

## Negatives

Each reports one error at the reference ("cannot refer to an element or
member of a dynamic type outside of a procedural context (IEEE 1800-2009
6.21; 25.9 for a virtual interface)"):

| Fixture | Location | Form |
| --- | --- | --- |
| `neg_assign_queue_element` | 6:14 | `assign x = pq[1]` |
| `neg_assign_dynamic_element` | 6:14 | `assign x = da[0]` |
| `neg_net_declaration_associative_element` | 5:19 | `wire [31:0] w = aa["k"]` |
| `neg_input_port_queue_element` | 9:15 | `.s(sq[i])`, a string port and a variable index |
| `neg_input_port_associative_element` | 8:15 | `.v(aa[7])` |
| `neg_gate_queue_element` | 6:13 | `and g (o, pq[0], 1'b1)` |
| `neg_assign_class_property` | 9:14 | `assign x = h.a`, a class property |
| `neg_assign_virtual_interface_member` | 10:14 | `assign x = v.s` (25.9) |
| `neg_assign_record_element_member` | 9:14 | `assign x = rq[0].a` |
| `neg_assign_nested_queue_element` | 6:14 | `assign x = fq[0][0]`, a queue in a fixed array |
| `neg_assign_call_actual` | 9:16 | `assign x = f(pq[1])`, an actual of a call |
| `neg_assign_string_byte` | 6:14 | `assign b = s[0]`, a byte of a string |
| `neg_process_status_continuous` | 5:31 | `wire done = ... p.status() ...`, a method of a process object (formerly a listed limit) |
| `neg_randomize_continuous` | 9:14 | `assign w = h.randomize()`, a method of a class object (formerly a listed limit) |

A whole dynamic variable read by a method or query (`assign n = q.size()`)
is no element or member: it stays accepted. The industry-standard simulator
rejects it as well; the text limits only elements and members.
Procedural reads of the same elements (`always_comb`, `always @*`) are
legal and run (`array_sensitivity/containers.sv`, `edge_paths.sv`).
