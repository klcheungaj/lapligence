// ADV-026 A02: the first argument must be a memory.
module tb;
  logic [7:0] word;
  initial $sreadmemh(word, 0, 0, "11");
endmodule
