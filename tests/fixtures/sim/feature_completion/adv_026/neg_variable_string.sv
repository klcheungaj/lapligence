// ADV-026 decision S26-D17: the data arguments are string literals (IEEE 1800-2009
// D.14); a string variable is rejected with its location.
module tb;
  logic [7:0] mem [0:3];
  string image = "11 22";
  initial $sreadmemh(mem, 0, 1, image);
endmodule
