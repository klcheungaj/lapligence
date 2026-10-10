// Decision S14-D2: an intra-assignment or `->>` repeat event control whose
// count is zero, negative (signed) or contains X/Z does not wait at all: a
// blocking assignment completes at once, a nonblocking assignment or
// nonblocking trigger is scheduled in the current time step.
//
// IEEE 1800-2009 9.4.5 (SystemVerilog-1800-2009.txt L12140-12149): "If the
//   repeat count literal, or signed variable holding the repeat count, is
//   less than or equal to 0 at the time of evaluation, the assignment occurs
//   as if there is no repeat construct. ... repeat (-3) @ (event_expression)
//   // will not execute event_expression."
// 12.7.2 (L18159-18160), for the repeat loop: "If the expression evaluates to
//   unknown or high impedance, it shall be treated as zero".
//
// llg reads "as if there is no repeat construct" together with the example
// as "no event is waited for" (not as a single `@(...)` wait), and applies
// the repeat-loop X/Z rule to repeat event controls.
`timescale 1ns / 1ns
module tb;
  event e, t;
  int n;
  logic [3:0] xn;
  int a, b;

  initial forever #2 ->e;

  always @t $display("%0t t", $time);

  initial begin
    #1 n = 0;
    a = repeat (n) @e 1;
    $display("%0t a=%0d", $time, a);
    n = -3;
    a = repeat (n) @e 2;
    $display("%0t a=%0d", $time, a);
    xn = 4'bx;
    a = repeat (xn) @e 3;
    $display("%0t a=%0d", $time, a);
    ->> repeat (n) @e t;
    #2 $display("%0t b=%0d", $time, b);
    $finish;
  end

  initial #1 b <= repeat (-1) @e 4;
endmodule
