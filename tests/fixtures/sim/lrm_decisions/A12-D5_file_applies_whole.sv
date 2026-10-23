// Decision A12-D5: an SDF file is annotated as a whole or not at all. A
// file with a syntax error annotates nothing, not even the entries before
// the error, and a call with an invalid mtm_spec annotates nothing; each is
// reported with a warning and simulation continues with the earlier delays.
//
// IEEE 1800-2009 32.3 (SystemVerilog-1800-2009.txt L52627-52628): "It shall
// issue a warning for any data it is unable to annotate." The text does not
// say whether the entries before an error are annotated.
`timescale 1ns/1ps
module BUF (input a, output y);
  buf b (y, a);
  specify
    (a => y) = (1, 1);
  endspecify
endmodule
module tb;
  reg a = 1'b0;
  wire y, z;
  BUF u (a, y);
  BUF v (a, z);
  initial begin
    #5 $monitor("%0t y=%b z=%b", $realtime, y, z);
    #5 $sdf_annotate("A12-D5_file_applies_whole.sdf");
    $sdf_annotate("A12-D5_file_applies_whole_v.sdf", , , , "NOMINAL");
    #10 a = 1'b1;
    #20 $finish;
  end
endmodule
