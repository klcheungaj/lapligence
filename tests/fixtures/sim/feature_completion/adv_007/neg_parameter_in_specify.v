// A parameter may not be used inside a specify block (decision A7-D5).
module m(input a, output y);
  parameter P = 2;
  assign y = a;
  specify
    (a => y) = P;
  endspecify
endmodule
module tb;
  reg a;
  wire y;
  m u(.a(a), .y(y));
  initial #1 $finish;
endmodule
