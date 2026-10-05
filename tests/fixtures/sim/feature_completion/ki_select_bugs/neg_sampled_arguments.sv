// $sampled takes exactly one expression (IEEE 1800-2009 16.9.3).
module tb;
  logic [7:0] x;
  initial $display("%h", $sampled(x, x));
endmodule
