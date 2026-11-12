// ADV-013 A02: the 16.13.17 example `fibonacci1` (L25533-25541), with
// overlapping attempts whose actuals are sampled where each instance starts
// (`|->` at the attempt, `|=>` one tick later). Expected lines are derived
// in readme.md.
`timescale 1ns/1ns
module tb;
  logic clk = 0, start = 0;
  int fib = 0, bad = 0, a0 = 0, b0 = 0, n0 = 0;
  always #5 clk = ~clk;
  property fibonacci1 (local input int a, b, n, int fib_sig);
    (n > 0)
    |->
    (
      (fib_sig == a)
      and
      (1'b1 |=> fibonacci1(b, a + b, n - 1, fib_sig))
    );
  endproperty
  f1: assert property (@(posedge clk) start |-> fibonacci1(a0, b0, n0, fib))
        $display("f1 P %0t", $time); else $display("f1 F %0t", $time);
  f2: assert property (@(posedge clk) start |-> fibonacci1(a0, b0, n0, bad))
        $display("f2 P %0t", $time); else $display("f2 F %0t", $time);
  f3: assert property (@(posedge clk) start |=> fibonacci1(a0, b0, 2, fib))
        $display("f3 P %0t", $time); else $display("f3 F %0t", $time);
  initial begin
    #12 start = 1'b1; a0 = 0; b0 = 1; n0 = 6; fib = 0; bad = 0;
    #10 start = 1'b0; fib = 1; bad = 1; a0 = 1; b0 = 1;
    #10 start = 1'b1; a0 = 1; b0 = 2; n0 = 3; fib = 1; bad = 1;
    #10 start = 1'b0; fib = 2; bad = 2; a0 = 2; b0 = 3;
    #10 fib = 3; bad = 4;
    #10 fib = 5; bad = 5;
    #10 fib = 8; bad = 8;
    #10 fib = 13; bad = 13;
    #20 $finish;
  end
endmodule
