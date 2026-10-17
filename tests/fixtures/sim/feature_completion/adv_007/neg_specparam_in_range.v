// A specparam may not appear in a declaration range (the frontend reports it).
module m(input a, output y);
  specparam S = 2;
  wire [S:0] w;
  assign y = a;
endmodule
module tb;
  reg a;
  wire y;
  m u(.a(a), .y(y));
  initial #1 $finish;
endmodule
