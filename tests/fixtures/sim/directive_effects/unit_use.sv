// IEEE 1364-2001 section 19.4 and IEEE 1800-2009 section 22.6: a later source
// observes a definition only in merged compilation-unit mode.
`ifdef SYN017_UNIT_FLAG
`define SYN017_UNIT_VALUE 1
`else
`define SYN017_UNIT_VALUE 0
`endif
module tb;
  initial begin
    $display("unit_flag=%0d", `SYN017_UNIT_VALUE);
    $finish;
  end
endmodule
