// RTL-012 negative: a bit of a vector net is not a scalar net, so an explicit
// continuous drive strength is prohibited (IEEE 1800-2009 10.3.4). Gate
// strengths on the same bit stay legal (see wide_composition).
module tb;
  wire [3:0] w;
  assign (weak1, weak0) w[1] = 1'b1;
  initial begin #1 $display("%b", w); $finish(0); end
endmodule
