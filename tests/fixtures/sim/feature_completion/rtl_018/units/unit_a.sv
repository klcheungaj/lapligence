// RTL-018 library `rtl`: defines a macro for later sources of the same unit.
`define RTL018_SHARED 7
`define RTL018_LATE_HEADER "late.vh"
`include "mark.vh"
module rtl018_unit_a(mark);
  output [7:0] mark;
  assign mark = `RTL018_MARK;
endmodule
