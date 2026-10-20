// A specparam may not appear in a parameter value (the frontend reports it).
module m(input a, output y);
  specparam S = 2;
  parameter P = S;
  assign y = a;
endmodule
module tb;
  reg a;
  wire y;
  m u(.a(a), .y(y));
  initial #1 $finish;
endmodule
