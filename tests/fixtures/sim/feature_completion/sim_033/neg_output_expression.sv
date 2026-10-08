// SV 14.5: an output clocking expression must be a legal output port
// connection, so a computed value is illegal.
module tb;
  logic clk;
  int a, b;
  clocking cb @(posedge clk);
    output s = a + b;
  endclocking
endmodule
