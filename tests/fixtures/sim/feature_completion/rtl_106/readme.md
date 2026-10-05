# RTL-106 source-provenance fixtures

## Mapped frontend diagnostics

A `` `line N "f" L`` directive gives the next physical line logical line N in
file f (IEEE 1364-2001 19.7, IEEE 1800-2009 22.12). Positions are counted by
hand from the physical lines:

- `line_slang_error.sv`: directive on line 2, so the undeclared name on line 5
  is `orig_rtl.sv:42`.
- `line_macro_error.sv`: the macro body's error reports at its use site,
  line 6, two lines after the directive on line 4: `gen_top.sv:201`. The
  warning on line 3 precedes the directive and stays physical.
- `line_include_error.sv` includes `line_include_error.svh`, whose own
  directive on line 2 maps line 3 to `orig_header.svh:7`.
- `line_lint.v`: the directive on line 5 maps the `case` on line 7 to
  `orig_lint.v:31` in the lint text and `--lint-json` reports.
- `neg_macro_line.v`: a macro-built end label on line 6 after a directive on
  line 3 reports `orig_rtl.v:72`.

## Strict 2001 profile

Each `neg_*.v` is legal SystemVerilog-2009 and legal 1364-2001 except for one
form: `neg_macro_*` build a later form through a macro (reported at the macro
use site), the others are keyword-free later grammar or a variable driver
that IEEE 1364-2001 6.1, 7.1 and 12.3.9.2 forbid (`neg_assign_reg.v` is the
RTL-105 finding).

`legal_2001.v` is the nearest legal composition: named generate loop blocks,
`parameter` ports, declared genvars, ranged memories, UDP, gate, `defparam`,
named blocks and calls built by macros, `fork`/`join`, `force`/`release`,
events, intra-assignment NBA, `casez`, `disable`, `wait` and an edge-list
event control. `legal_2001.out` follows from the statements: for example
`sum = 3 + 4`, `y4 = 4'h3 ^ 4'h5`, `fact(5) = 120`, the fork arms finish at
times 2 and 3 after starting at 1, and `hits` counts `posedge p` at 8 and
`negedge q` at 9.
