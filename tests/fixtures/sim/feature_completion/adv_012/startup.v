// ADV-012 startup: SDF files named on the command line (`--sdf
// [min:|typ:|max:][scope=]file`) are annotated, in order, before
// simulation starts, so the transitions at time 0 already use them
// (IEEE 1800-2009 32.1 leaves how an annotator is invoked to the tool).
// Oracles in readme.md.
`timescale 1ns/1ps

module BUF (input a, output y);
  buf b0 (y, a);
  specify
    (a => y) = (1, 1);
  endspecify
endmodule

module tb;
  reg a = 1'b0;
  wire [2:0] y;
  BUF u0 (a, y[0]);
  BUF u1 (a, y[1]);
  BUF u2 (a, y[2]);
  initial begin
    $monitor("%0t a=%b y=%b", $realtime, a, y);
    #20 a = 1'b1;
    #20 a = 1'b0;
    #20 $finish;
  end
endmodule
