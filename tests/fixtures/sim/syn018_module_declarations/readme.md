# SYN-018 module declarations

`module_declarations.sv` is the positive SystemVerilog-2009 witness. It uses
two parent modules with independently scoped nested `leaf` definitions. Each
leaf reads its parent instance's `BASE` parameter, is instantiated in a
parameter-selected generate scope, and applies its own `EXTRA` override.
`extern_child.sv` and `extern_child_body.sv` exercise a parameterized
extern declaration and matching body across separate source files.

The public CLI suite runs the positive witness with separate and merged
compilation-unit policies, in optimized and unoptimized modes. The owned model
assertions check the two nested instance paths, their distinct parameter values,
and the parameterized extern instance. `extern_mismatch.sv` and
`extern_missing.sv` each contain one frontend fault and remain rejection
controls. `extern_2001.sv` keeps the SystemVerilog-only extern form separate
from the body and signature controls.

The source forms are mapped to IEEE 1800-2009 §§23.4–23.5 in
[`docs/specification/spec-reference-sv.md`](../../../../docs/specification/spec-reference-sv.md).
This suite does not cover configurations or library search, and it does not
claim support for module features introduced after the selected 2009 profile.
