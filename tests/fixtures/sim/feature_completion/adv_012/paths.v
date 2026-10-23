// ADV-012 paths: IOPATH delays, pulse limits from delvals, PATHPULSE and
// PATHPULSEPERCENT, and INCREMENT annotations applied while simulating
// (IEEE 1800-2009 32.4.2, 32.6, 32.7, 32.8; IEEE 1364-2001 16.2). Oracles
// in readme.md.
`timescale 1ns/1ps

// Cell library: every path delay starts at 1.
module BUF (input a, output y);
  buf b0 (y, a);
  specify
    (a => y) = (1, 1);
  endspecify
endmodule

module tb;
  reg a = 1'b0;
  wire [5:0] y;
  BUF u0 (a, y[0]);
  BUF u1 (a, y[1]);
  BUF u2 (a, y[2]);
  BUF u3 (a, y[3]);
  BUF u4 (a, y[4]);
  BUF u5 (a, y[5]);
  initial begin
    // Before any timing control: the time-0 transitions use the file.
    $sdf_annotate("paths.sdf");
    $monitor("%0t a=%b y=%b", $realtime, a, y);
    #20 a = 1'b1;
    #20 a = 1'b0;
    // Pulses of 2, 6 and 9 ns.
    #60 a = 1'b1;
    #2 a = 1'b0;
    #98 a = 1'b1;
    #6 a = 1'b0;
    #94 a = 1'b1;
    #9 a = 1'b0;
    // u0 is due to rise at 401 when the first INCREMENT applies at 400.
    #87 a = 1'b1;
    #4 $sdf_annotate("inc.sdf", tb);
    #20 a = 1'b0;
    #20 a = 1'b1;
    #20 $sdf_annotate("inc.sdf", tb);
    #20 a = 1'b0;
    #20 a = 1'b1;
    #20 $sdf_annotate("inc.sdf", tb);
    #20 a = 1'b0;
    #20 a = 1'b1;
    #40 $finish;
  end
endmodule
