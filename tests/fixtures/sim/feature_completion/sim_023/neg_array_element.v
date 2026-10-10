// V2001 9.3.2: a force target cannot be a memory word (array reference).
module tb;
  reg [3:0] mem [0:3];
  initial force mem[1] = 4'h1;
endmodule
