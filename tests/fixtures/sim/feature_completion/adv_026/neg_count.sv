// ADV-026 A02: memory, start, finish and at least one string are required.
module tb;
  logic [7:0] mem [0:3];
  initial $sreadmemh(mem, 0, 3);
endmodule
