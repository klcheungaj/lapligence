// ADV-026 A02: the argument order differs from $readmemh; the file name comes
// first there and the memory comes first here.
module tb;
  logic [7:0] mem [0:3];
  initial $sreadmemh("11 22", mem, 0, 1);
endmodule
