// Multidimensional memory files are an IEEE 1800 addition (21.4.3).
module tb;
  reg [7:0] mem [0:1][0:3];
  initial $readmemh("in.hex", mem);
endmodule
