// $fatal (IEEE 1800-2009 20.10), with and without a finish number.
`timescale 1ns/1ps
module tb;
  $fatal;
  $fatal(0);
  $fatal(2, "W=%0d", 12);
  initial $display("must not run");
endmodule
