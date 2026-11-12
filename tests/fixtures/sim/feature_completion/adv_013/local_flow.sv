// ADV-013 A02: a local assigned by an outer antecedent flows through a nested
// implication whose antecedent assigns another local (16.10 local variable
// flow). Expected lines are derived in readme.md.
`timescale 1ns/1ns
module tb;
  logic clk = 0, a = 0, b = 0;
  logic [3:0] d = 0, e = 0;
  always #5 clk = ~clk;
  property pr;
    logic [3:0] x, y;
    (a, x = d) |=> ((b, y = e) |=> (x == y + 4'd1));
  endproperty
  p1: assert property (@(posedge clk) pr) $display("p1 P %0t", $time); else $display("p1 F %0t", $time);
  initial begin
    #12 a = 1'b1; d = 4'd5;
    #10 a = 1'b0; b = 1'b1; e = 4'd4;
    #10 b = 1'b0;
    #10 a = 1'b1; d = 4'd7;
    #10 a = 1'b0; b = 1'b1; e = 4'd2;
    #10 b = 1'b0;
    #20 $finish;
  end
endmodule
