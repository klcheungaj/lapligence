// ADV-026 A02: a multidimensional memory is not Verilog-2001.
module tb;
  reg [7:0] mem [0:1][0:1];
  initial $sreadmemh(mem, 0, 1, "11 22");
endmodule
