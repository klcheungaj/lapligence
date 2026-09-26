// IEEE 1364-2001 19.3-19.4: parameter substitution, generated ranges,
// undef, and conditional branch selection affect executable storage.
`include "legacy_header.svh"
`define SYN017_RANGE(msb,lsb) [msb:lsb]
`define SYN017_VALUE(x) ((x) + 1)
`define SYN017_SWITCH
`undef SYN017_SWITCH
`ifndef SYN017_SWITCH
`define SYN017_BRANCH `SYN017_FROM_HEADER
`else
`define SYN017_BRANCH 9
`endif
module tb;
  reg `SYN017_RANGE(6,0) value;
  initial begin
    value = `SYN017_VALUE(`SYN017_BRANCH);
    $display("value=%b", value);
    $finish;
  end
endmodule
