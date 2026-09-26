# SYN-018 module declarations

`module_declarations.sv` is the positive SystemVerilog-2009 witness. It uses
two parent modules with independently scoped nested definitions. Each
`captured_base` reads its parent's `BASE` parameter from a same-scope instance.
Each generated `leaf` connects the enclosing input `a` and output `y` directly,
receives the captured `BASE` contribution through a separate port, and applies
its own `EXTRA` override.
`extern_child.sv` and `extern_child_body.sv` exercise a parameterized
extern declaration and matching body across separate source files.
`extern_specializations.sv` instantiates that pair at widths four and five;
`nested_specializations.sv` instantiates one enclosing definition twice, with
different enclosing parameter values, nested defaults and port widths. Its
local `leaf` is instantiated in the same scope as its declaration.

The public CLI suite runs the three positive witnesses with separate and merged
compilation-unit policies, in both optimizer modes. The owned model
assertions check the nested instance paths, parameter values and port widths,
and the parameterized extern instances. The DUT-only tops also pass owned
simulation/synthesis classification and code generation after native snapshot
destruction. `extern_mismatch.sv` and
`extern_missing.sv` each contain one frontend fault and remain rejection
controls. `nested_out_of_scope.sv` checks that the local definition cannot be
instantiated from outside its parent. `nested_declaration_in_generate.sv`
checks the Annex A.1.4/A.4.2 grammar boundary: generated instances are legal,
but a module declaration directly in a generate block is not a generate item.
`extern_2001.sv` and `nested_2001.sv` keep the SystemVerilog-only forms
separate from the 2009 controls.

The source forms are mapped to IEEE 1800-2009 §§23.4–23.5 in
[`docs/specification/spec-reference-sv.md`](../../../../docs/specification/spec-reference-sv.md).
This suite does not cover configurations or library search, and it does not
claim support for module features introduced after the selected 2009 profile.
