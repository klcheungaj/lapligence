// A variable-position slice wider than its dimension can never lie inside the
// array (IEEE 1800-2009 7.4.6: the size is constant); rejected when compiled.
module tb;
  logic [7:0] mem [0:7];
  int i = 1;
  initial $readmemh("words.mem", mem[i+:9]);
endmodule
