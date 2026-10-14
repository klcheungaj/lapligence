// $writememh is an IEEE 1800 task; Verilog-2001 has only the loads (17.2.8).
module tb;
  reg [7:0] mem [0:3];
  initial $writememh("out.hex", mem);
endmodule
