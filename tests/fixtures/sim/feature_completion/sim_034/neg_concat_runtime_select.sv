`timescale 1ns/1ns
// SV 1800-2009 14.16: a runtime select of a clockvar bound to a concatenation
// is legal; llg rejects it (FIN-001 K2).
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  logic [3:0] a = 4'h0, b = 4'h0;
  int i = 1;
  clocking cb @(posedge clk);
    output y = {a, b};
  endclocking
  initial begin
    #6 cb.y[i] <= 1'b1;
    #10 $display("%0t a=%b b=%b", $time, a, b);
    $finish;
  end
endmodule
