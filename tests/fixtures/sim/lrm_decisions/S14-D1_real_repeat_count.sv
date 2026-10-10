// Decision S14-D1: a real repeat count (repeat loop or repeated event
// control) converts to an integer by rounding, ties away from zero, and a
// result of zero or less runs zero times.
//
// IEEE 1800-2009 9.4.5 (SystemVerilog-1800-2009.txt L12140-12141): "If the
//   repeat count literal, or signed variable holding the repeat count, is
//   less than or equal to 0 at the time of evaluation, the assignment occurs
//   as if there is no repeat construct."
// 12.7.2 (L18159-18160) does not mention real counts. 6.12.2 (L5531-5533):
//   "Real numbers shall be converted to integers by rounding the real number
//   to the nearest integer, rather than by truncating it. ... If the
//   fractional part of the real number is exactly 0.5, it shall be rounded
//   away from zero."
//
// The text is silent on real counts; llg applies the 6.12.2 conversion.
// Expected: 2.5 waits three events (2, 4, 6); -1.5 rounds to -2 and waits
// none; 1.4 waits one event (8).
`timescale 1ns / 1ns
module tb;
  event e;
  real r;
  int a;

  initial forever #2 ->e;

  initial begin
    #1 r = 2.5;
    repeat (r) @e;
    $display("%0t repeat 2.5", $time);
    #1 r = -1.5;
    repeat (r) @e;
    $display("%0t repeat -1.5", $time);
    r = 1.4;
    a = repeat (r) @e 5;
    $display("%0t a=%0d", $time, a);
    $finish;
  end
endmodule
