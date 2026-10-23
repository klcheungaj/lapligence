// Decision A12-D8: an INTERCONNECT whose source port does not exist still
// annotates its load port, as the delay from every source, with a warning.
//
// IEEE 1800-2009 32.4.4 (SystemVerilog-1800-2009.txt L52917-52921): "If the
// source port is not found or if the source port and the load port are not
// actually on the same net, then a warning is issued, but the delay to the
// load port is annotated anyway. If this happens for a load port that is
// part of a multisource net, then the delay is treated as if it were the
// delay from all source ports, which is the same as the annotation behavior
// for a PORT delay."
`timescale 1ns/1ps
module BUF (input a, output y);
  buf b (y, a);
  specify
    (a => y) = (1, 1);
  endspecify
endmodule
module tb;
  reg a = 1'b0;
  wire n, y;
  BUF s (a, n);
  BUF l (n, y);
  initial begin
    #10 $sdf_annotate("A12-D8_interconnect_unknown_source.sdf");
    #10 a = 1'b1;
    #20 $finish;
  end
  always @(y) if ($realtime > 0) $display("%0t y=%b", $realtime, y);
endmodule
