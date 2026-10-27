// ADV-015 A03: a delay control in a checker always procedure (17.5).
`timescale 1ns / 1ns
checker bad(logic a, event clk);
  bit b;
  always begin
    #1 b <= a;
  end
endchecker
module tb;
  logic clk = 1'b0, x = 1'b0;
  bad u(x, posedge clk);
  initial #10 $finish;
endmodule
