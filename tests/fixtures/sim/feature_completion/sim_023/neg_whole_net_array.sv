// Not supported by llg: a whole unpacked net array; its elements are.
module tb;
  logic [3:0] d;
  wire [3:0] na [0:1];
  assign na[0] = d;
  assign na[1] = ~d;
  initial force na = '{4'h1, 4'h2};
endmodule
