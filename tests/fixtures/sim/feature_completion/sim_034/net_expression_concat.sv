`timescale 1ns/1ns
// SV 1800-2009 14.5, 14.16: clockvars bound to concatenations over wires and
// a variable, an inout bound to a select of a wire, a skewed output, and a
// constant select of a concatenation-bound clockvar (FIN-001 K2).
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  wire [7:0] w;
  wire [3:0] w2;
  logic [3:0] v = 4'h0;
  wire [7:0] w3;
  logic [7:0] d3 = 8'hzz;
  assign w3 = d3;
  logic [3:0] hi = 4'hz;
  assign w[7:4] = hi;
  clocking cb @(posedge clk);
    output y = {w[7:6], w[1:0]};
    output m = {w2, v};
    inout z = w3[3:0];
    output #2 k = w[3:2];
  endclocking
  initial begin
    #6 cb.y <= 4'b1001;
    cb.m <= 8'hA5;
    cb.z <= 4'h6;
    cb.k <= 2'b11;
    #10 $display("%0t w=%b w2=%b v=%b w3=%b", $time, w, w2, v, w3);
    hi = 4'b0110;
    d3 = 8'h31;
    #1 $display("%0t w=%b w3=%b", $time, w, w3);
    #9 $display("%0t cb.z=%b", $time, cb.z);
    cb.y[2] <= 1'b1;
    cb.m[5:2] <= 4'b0110;
    cb.z[3] <= 1'b1;
    #10 $display("%0t w=%b w2=%b v=%b w3=%b cb.z=%b", $time, w, w2, v, w3, cb.z);
    $finish;
  end
endmodule
