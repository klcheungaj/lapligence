// ADV-026 A02: a net array is not a variable memory.
module tb;
  wire [7:0] mem [0:3];
  initial $sreadmemh(mem, 0, 1, "11 22");
endmodule
