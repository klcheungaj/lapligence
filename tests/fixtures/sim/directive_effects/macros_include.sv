// IEEE 1364-2001 sections 19.3.1, 19.4, 19.5 and IEEE 1800-2009
// sections 22.4–22.6:
// include guards, argument macros, token concatenation, stringification and
// conditional branches reach the executable design.
`include "macro_header.svh"
`include "macro_header.svh"
`ifdef ENABLE
`define SYN017_BRANCH 11
`elsif ALT
`define SYN017_BRANCH 22
`else
`define SYN017_BRANCH 33
`endif
module tb;
  reg [`SYN017_WIDTH-1:0] `SYN017_CAT(foo,bar);
  initial begin
    `SYN017_CAT(foo,bar) = `SYN017_WIDTH'hA;
    $display("branch=%0d cat=%0h text=%s", `SYN017_BRANCH,
             `SYN017_CAT(foo,bar), `SYN017_STR(syn017));
    $finish;
  end
endmodule
