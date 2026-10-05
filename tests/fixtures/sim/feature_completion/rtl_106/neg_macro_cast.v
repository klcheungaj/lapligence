`define WIDEN(x) 8'(x)
module tb;
  reg [7:0] r;
  initial r = `WIDEN(4'd3);
endmodule
