// IEEE 1800-2009 22.5.3; undefineall does not exist in 1364-2001.
`define SYN017_GONE
`undefineall
`ifndef SYN017_GONE
`define SYN017_RESULT 1
`else
`define SYN017_RESULT 0
`endif
module tb;
  initial begin
    $display("undefineall=%0d", `SYN017_RESULT);
    $finish;
  end
endmodule
