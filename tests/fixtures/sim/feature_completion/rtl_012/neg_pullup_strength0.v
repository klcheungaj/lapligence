// RTL-012 negative: a pullup takes only a strength1 specification
// (IEEE 1364-2001 7.1.2).
module tb;
  wire w;
  pullup (strong0) p(w);
  initial begin #1 $display("%b", w); $finish(0); end
endmodule
