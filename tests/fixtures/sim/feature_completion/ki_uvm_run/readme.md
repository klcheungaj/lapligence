# Forms a verification library's run reaches

The task module runs every fixture through the public CLI in both HDL optimizer
modes on legacy, compact/portable and compact/GMP values; the generated-C sweep
(`tests/generated_c_frame_lint.rs`) compiles each one with `-Wall` and accepts
no warning. Expected outputs were checked against an independent
industry-standard simulator; the clause lines below are from
`SystemVerilog-1800-2009.txt`.

| Fixture | Clause and oracle |
| --- | --- |
| `unused_locals` | 6.21 L6901-6903: "Variables declared in an automatic task, function, or block are local in scope, default to the lifetime of the call or block and are initialized on each entry to the call or block". 8.6 L9766: "The lifetime of methods declared as part of a class type shall be automatic." String, class handle, chandle and process locals of class methods, automatic functions and an automatic task that nothing names after the declaration (a handle local beside a `$cast` destination, as in a library's `do_copy`); each call returns as if they were absent. |
