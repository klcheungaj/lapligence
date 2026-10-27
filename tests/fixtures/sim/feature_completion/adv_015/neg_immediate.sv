// ADV-015 A03: a simple immediate assertion in a checker always procedure (17.5).
`timescale 1ns / 1ns
checker bad(logic a, event clk);
  always @(clk) begin
    assert (a);
  end
endchecker
module tb;
  logic clk = 1'b0, x = 1'b0;
  bad u(x, posedge clk);
  initial #10 $finish;
endmodule
