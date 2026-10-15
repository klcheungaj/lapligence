// Decision S39-D9: a local input formal of a property is initialized when
// the property's evaluation attempt begins and keeps that value in the
// consequent of a non-overlapped implication.
//
// IEEE 1800-2009 16.10 (SystemVerilog-1800-2009.txt L23800-23807): "At the
// beginning of each evaluation attempt of an instance of a named sequence or
// property, a new copy of each of its local variables shall be created and,
// if present, the corresponding initialization assignment shall be performed.
// ... Non-local variables appearing in the expression of an initialization
// assignment to a local variable shall be evaluated using the Preponed values
// from the time slot in which the evaluation attempt begins."
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, a = 1'b0;
  logic [7:0] d = 8'd0;
  property held(local input logic [7:0] v);
    a |=> d == v + 8'd1;
  endproperty
  always #5 clk = ~clk;
  a1: assert property (@(posedge clk) held(d))
    else $display("%0t fail", $time);
  c1: cover property (@(posedge clk) held(d)) $display("%0t pass", $time);
  initial begin
    #2 a = 1'b1; d = 8'd1;       // 5: v = 1
    #10 d = 8'd2;                // 15: holds (2 == 1 + 1); v = 2
    #10 a = 1'b0; d = 8'd4;      // 25: fails (4 != 2 + 1)
    #10 $finish;
  end
endmodule
