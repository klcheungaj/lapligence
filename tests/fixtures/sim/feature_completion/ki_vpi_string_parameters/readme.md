# String parameters as VPI system task arguments

Run by [`tests/sim_feature_completion/ki_vpi_string_parameters.rs`](../../../../sim_feature_completion/ki_vpi_string_parameters.rs)
in both optimizer modes on every value backend. Clause line numbers refer to
the `pdftotext -layout` extraction `SystemVerilog-1800-2009.txt`, which is not
part of the repository.

- 37.26 L59398: "For a value parameter, vpi_get_value() shall return the
  value that the parameter has at the end of elaboration."
- 37.38 (tf call, L60032): `vpiArgument` iterates over the `expr` arguments of a
  call.
- 38.15 L62779-62780: "If a constant object's vpiConstType is vpiStringVal, the
  value shall be retrieved using a format of either vpiStringVal or
  vpiVectorVal."

| Case | Covers |
| --- | --- |
| `arguments.sv` + `arguments.c` | `string` parameters of a module, an instance (default and overridden), a block and a task, an empty string, hierarchical names and a literal as arguments of one system task: object type, constant type, full name and `vpiStringVal` |

A string variable or expression stays a located error
([`neg_vpi_string_variable`](../sim_099/readme.md)).
