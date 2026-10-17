// ADV-026 A02: a memory slice is not Verilog-2001.
module tb;
  reg [7:0] mem [0:7];
  initial $sreadmemh(mem[1:2], 1, 2, "11 22");
endmodule
