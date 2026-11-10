`timescale 1ns/1ns
// SV 1800-2009 14.5, 14.16: a clockvar bound to a concatenation of bits that a
// port connection merges into a shared net (A21-D5) and a variable drives each
// operand through its own target (FIN-001 K2).
module child (output wire [3:0] o, input logic [3:0] d);
  assign o = d;
endmodule
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  wire [7:0] w;
  logic [3:0] d = 4'hz;
  logic [1:0] v = 2'b00;
  child c (.o(w[3:0]), .d(d));
  clocking cb @(posedge clk);
    output y = {w[3:2], v};
  endclocking
  initial begin
    #6 cb.y <= 4'b1111;
    #10 $display("%0t w=%b v=%b", $time, w, v);
    $finish;
  end
endmodule
