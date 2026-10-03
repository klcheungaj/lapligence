// SV2009 11.4.2: increment/decrement in a continuous assignment is illegal.
module tb;
  int x, y;
  assign y = x++;
endmodule
