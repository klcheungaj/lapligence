// ADV-026 decision S26-D17: a concatenation of literals is an expression, not a
// string literal; the second data argument is the one reported.
module tb;
  logic [7:0] mem [0:3];
  initial $sreadmemh(mem, 0, 1, "11", {"2", "2"});
endmodule
