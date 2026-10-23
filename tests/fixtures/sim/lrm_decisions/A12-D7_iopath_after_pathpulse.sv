// Decision A12-D7: an IOPATH annotated after a PATHPULSE replaces the pulse
// limits the PATHPULSE gave with limits derived from the new delay (100% by
// default), so a 6 ns pulse through a 10 ns path is rejected.
//
// IEEE 1800-2009 32.5 (SystemVerilog-1800-2009.txt L52973-52974): "The
// following example first annotates pulse limits to an IOPATH and then
// annotates the entire IOPATH, thereby overwriting the pulse limits that
// were just annotated"; 32.7 (L53033-53034): "the default values annotated
// for pulse limits shall be calculated using the percentage settings for
// the reject and error limits. By default, these limits are 100%".
`timescale 1ns/1ps
module BUF (input a, output y);
  buf b (y, a);
  specify
    (a => y) = (1, 1);
  endspecify
endmodule
module tb;
  reg a = 1'b0;
  wire y;
  BUF u (a, y);
  initial begin
    #10 $sdf_annotate("A12-D7_iopath_after_pathpulse.sdf");
    #10 a = 1'b1;
    #6 a = 1'b0;
    #40 $finish;
  end
  always @(y) if ($realtime > 0) $display("%0t y=%b", $realtime, y);
  initial #50 $display("%0t final y=%b", $realtime, y);
endmodule
