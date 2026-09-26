// IEEE 1364-2001 19.4: command-line definitions select ordered branches.
`ifdef ENABLE
`define SYN017_CHOICE 11
`elsif ALT
`define SYN017_CHOICE 22
`else
`define SYN017_CHOICE 33
`endif
module tb;
  initial begin
    $display("choice=%0d", `SYN017_CHOICE);
    $finish;
  end
endmodule
