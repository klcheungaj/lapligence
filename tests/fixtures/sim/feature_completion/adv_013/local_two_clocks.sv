// ADV-013 A03: a property local with a declaration assignment in a property
// with two semantic leading clocks (16.14.7 L26329-26337): each leading
// clock has its own copy of `v`, initialized at that clock's first tick at or
// after the start of the evaluation. Expected lines are derived in readme.md.
`timescale 1ns/1ns
module tb;
  logic c1 = 0, c2 = 0, e = 0;
  logic [2:0] x = 0;
  property two_copies;
    logic [2:0] v = x;
    (@(posedge c1) v == 3'd4) and (@(posedge c2) v == 3'd3);
  endproperty
  p: assert property (@(posedge c1) e |=> two_copies)
       $display("p P %0t", $time); else $display("p F %0t", $time);
  initial begin
    // c1 ticks at 10, 30, 50, ...; c2 ticks at 16, 36, 56, ...
    fork
      forever #10 c1 = ~c1;
      begin #6 forever #10 c2 = ~c2; end
    join_none
    #8 e = 1'b1;           // sampled at c1 tick 10
    #4 e = 1'b0;           // 12
    #1 x = 3'd3;        // 13: c2 tick 16 samples 3
    #10 x = 3'd4;       // 23: c1 tick 30 samples 4
    #20 e = 1'b1;          // 43: attempt at 50, consequent at 70 (c1) and 76 (c2)
    #10 e = 1'b0;          // 53
    #14 x = 3'd4;       // 67
    #8 x = 3'd3;        // 75
    #30 $finish;
  end
endmodule
