# Parameters of procedural blocks

Run by [`tests/sim_feature_completion/ki_block_parameters.rs`](../../../../sim_feature_completion/ki_block_parameters.rs)
in both optimizer modes on every value backend. Clause line numbers refer to
the `pdftotext -layout` extraction `SystemVerilog-1800-2009.txt`, which is not
part of the repository.

Clauses used by every case:

- 9.3 / A.2.8 L11318-11323: "block_item_declaration ::= { attribute_instance }
  data_declaration | { attribute_instance } local_parameter_declaration ; |
  { attribute_instance } parameter_declaration ; ...".
- 9.3.4 L11572-11575: "A named block creates a new hierarchy scope. ... It allows
  local variables, parameters, and named events to be referenced hierarchically,
  using the block name."
- 6.20.4 L6771-6773: "Local parameters can be assigned constant expressions (see
  11.2.1) containing parameters, which in turn can be modified with defparam
  statements or instance parameter value assignments."

| Case | Covers |
| --- | --- |
| `kinds.sv` | untyped, sized, signed, real, shortreal, string, time and realtime parameters; as ranges, sizes, size casts and operands; one value per instance |
| `types.sv` | enumerations, packed structures and unions, unpacked arrays, type parameters and `typedef`s declared in the block |
| `scopes.sv` | unnamed and nested blocks, shadowing, loop, branch and `case` bodies, `fork` forms, `always` kinds, `final`, generate iterations, hierarchical names |
| `contexts.sv` | static and automatic initializers, `randcase`, `inside`, delays, selects, loops, `force`, `iff`, concurrent assertions, `disable fork`, streaming, an interface |
| `subroutines.sv` | class methods, functions and tasks; event controls, `wait`, constraints, `case` labels |
| `instances.sv` | per-instance values and block-local `typedef`s |
| `hierarchy.sv` | reads by hierarchical name from inside and outside the block |
| `vpi.sv` + `vpi.c` | `vpiParameter` iteration of block, fork and function scopes, `vpi_handle_by_name`, `vpiScope`, a system task argument (37.26) |
| `verilog.v` | the 2001 edition (1364-2001 A.2.8) |
| `neg_out_of_scope.sv` | a simple name outside the block is undeclared |
| `neg_assigned.sv` | a parameter is not assignable |

`fin <= #D 7` in `scopes.sv` at time 10 shows 7 at time 15; the final block
prints last.
