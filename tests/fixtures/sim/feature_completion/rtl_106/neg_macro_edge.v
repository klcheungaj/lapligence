`define ANY_EDGE(s) edge s
module tb;
  reg clk;
  always @(`ANY_EDGE(clk)) $display("edge");
endmodule
