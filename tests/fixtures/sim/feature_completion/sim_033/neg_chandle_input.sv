// SV 6.14, 14.5: a clocking input must be a legal input port connection and
// ports shall not have the chandle type.
module tb;
  logic clk;
  chandle c;
  clocking cb @(posedge clk);
    input c;
  endclocking
  initial $display("%0d", cb.c == null);
endmodule
