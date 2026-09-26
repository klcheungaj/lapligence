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

## Continuation matrix

| Fixture | Independent requirement |
| --- | --- |
| `legacy_specialization.sv` | Both editions: widths 4/7 after legal defparam overrides, signed outputs -1/-59, localparams 5/9, constant factorial(3)=6, rounded 4.5=5, negative packed index labels and generated lanes. The constant function does not read defparam-affected parameters. |
| `dependent_types.sv` | Three specializations with widths 4/7/4; distinct same-width enums select different generate branches; typed defaults, type()/typedef, default function arguments, real/string-derived extents, namespace re-export/header imports, parameterized interfaces and shadowed generate constants. |
| `constant_pattern_keys.sv` | Native constant casez with selector Z chooses key 1/width 4, while two runtime payloads (3c/c3) flow through nested keyed/default patterns. Neither a constant-only test nor a runtime-only case can replace this composition. |
| `unit_provider.sv`, `unit_consumer.sv` | Ordered merged inputs expose `$unit`; separate inputs and reversed merged order reject the value reference. |
| `package_consumer.sv` | A package declared in the provider remains visible under either file-grouping policy. |
| `zero_extent.sv`, `negative_extent.sv` | Single-expression unpacked sizes must be positive. Negative range labels remain legal in the legacy positive control. |
| `nominal_enum_mismatch.sv` | Equal-width distinct enums still require explicit conversion. |
| `capacity_extent.sv` | A language-valid 1048576-bit packed object reaches the exclusive backend capacity limit; this is not an edition/syntax error. |

`sim_syn016_elaboration.rs` owns exact public stdout, negative diagnostics,
owned-model width/signedness/branch checks and the file-boundary checks.
The newly added matrix is pending a configured Rust/Slang/public-CLI run.
It does not expand the Core profile to runtime native/string/real hardware,
nonterminating constant recursion or unsupported source productions.
