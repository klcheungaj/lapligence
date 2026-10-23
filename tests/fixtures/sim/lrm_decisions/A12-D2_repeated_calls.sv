// Decision A12-D2: every executed $sdf_annotate call annotates, also when
// the same call with the same file and scope runs twice in a row: two
// INCREMENT calls add their values twice.
//
// IEEE 1800-2009 32.6 (SystemVerilog-1800-2009.txt L53025-53027): "Each
// call to the $sdf_annotate task annotates the design with timing
// information from an SDF file. Annotated values either modify (INCREMENT)
// or overwrite (ABSOLUTE) values from earlier SDF files."
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
    #10 $sdf_annotate("A12-D2_repeated_calls.sdf");
    $sdf_annotate("A12-D2_repeated_calls.sdf");
    #10 a = 1'b1;
    #20 a = 1'b0;
    #20 $finish;
  end
  always @(y) if ($realtime > 0) $display("%0t y=%b", $realtime, y);
endmodule
