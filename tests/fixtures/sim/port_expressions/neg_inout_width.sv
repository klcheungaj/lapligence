// An inout port connection must not differ in width from its port
// expression: the bidirectional connection has no assignment that could
// widen or truncate a value.
module m(.p(a[2:0]));
  inout [3:0] a;
endmodule

module tb;
  wire [1:0] w;
  m c(.p(w));
endmodule
