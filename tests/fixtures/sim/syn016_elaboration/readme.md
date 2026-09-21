# Constant elaboration and namespace matrix

`elaboration_matrix.sv` exercises the selected SystemVerilog-2009 elaboration
surface from SYN-016: type and value parameters, typedef/type operator and enum
metadata, `$bits`/`$left`/`$right` queries, a terminating constant function and `let`,
folded real and string constants, package wildcard re-export, `$unit`, named
generate scopes, and parameterized interfaces. The public CLI runs it in both
optimizer modes with separate and merged compilation-unit policies.

`type_parameter_2001.sv` is a single-fault edition control: a type parameter
is rejected under the strict Verilog-2001 policy. `real_extent.sv` is a single-
fault range control: a real-valued parameter cannot be used as an integral
packed extent.

The matrix does not claim runtime real/string hardware, unbounded constant
recursion, arbitrary package/native layouts, or post-2009 language forms.
