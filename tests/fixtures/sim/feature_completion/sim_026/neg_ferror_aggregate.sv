module tb;
  integer e;
  reg [7:0] bytes [0:79];
  initial e = $ferror(0, bytes);
endmodule
