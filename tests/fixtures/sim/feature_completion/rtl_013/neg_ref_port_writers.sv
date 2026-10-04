// IEEE 1800-2009 9.2.2.4 and 23.3.3.3: two ref-port always_ff writers share one variable.
module w(ref logic [7:0] r, input logic c, input logic [7:0] d);
  always_ff @(posedge c) r <= d;
endmodule
module tb;
  logic [7:0] x; logic c; logic [7:0] d1, d2;
  w a(.r(x), .c(c), .d(d1));
  w b(.r(x), .c(c), .d(d2));
  initial $finish(0);
endmodule
