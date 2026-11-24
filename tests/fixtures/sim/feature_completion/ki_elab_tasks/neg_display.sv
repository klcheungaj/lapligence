// $display is no elaboration system task (IEEE 1800-2009 20.10).
`timescale 1ns/1ps
module tb;
  $display("illegal");
  initial $display("must not run");
endmodule
