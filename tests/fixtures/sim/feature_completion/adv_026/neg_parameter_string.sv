// ADV-026 decision S26-D17: a constant string parameter is not a string literal.
module tb;
  localparam string IMAGE = "11 22";
  logic [7:0] mem [0:3];
  initial $sreadmemh(mem, 0, 1, IMAGE);
endmodule
