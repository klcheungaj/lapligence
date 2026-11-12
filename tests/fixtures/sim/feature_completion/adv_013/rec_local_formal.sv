// ADV-013 A02: a recursive property with a local variable formal (16.13.17
// L25526-25530, restriction 4 third form). Each recursion level gets its
// own copy of `n`, initialized from the actual `n - 1` of the level before
// (16.8.2, 16.10). Expected lines are derived in readme.md.
`timescale 1ns/1ns
module tb;
  logic c1 = 0, a = 0, d = 1'b0;
  property count(local input int n);
    if (n > 0) (d and (1'b1 |=> count(n - 1)));
  endproperty
  p: assert property (@(posedge c1) a |-> count(2))
       $display("p P %0t", $time); else $display("p F %0t", $time);
  always #5 c1 = ~c1;
  initial begin
    #12 a = 1'b1; d = 1'b1;
    #10 a = 1'b0;
    #20 a = 1'b1;
    #10 a = 1'b0; d = 1'b0;
    #10 d = 1'b1;
    #20 $finish;
  end
endmodule
