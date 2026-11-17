// FIN-001 A3 limit: a concurrent assertion operand reading a cell of a fixed
// unpacked array (IEEE 1800-2009 16.6.1) needs the cell's sampled value,
// which arrays below the descriptor size do not keep; it is rejected at the
// operand.
module tb;
  logic clk = 1'b0;
  logic [3:0] arr[4] = '{4'h1, 4'h2, 4'h3, 4'h4};
  int i = 1;
  always #5 clk = ~clk;
  a1: assert property (@(posedge clk) arr[i] == 4'h2);
  initial #20 $finish;
endmodule
