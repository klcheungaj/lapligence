// A variable destination is legal (SV 30.4.1) but not simulated yet.
module m(input a, output reg y);
  always @(a) y = a;
  specify
    (a => y) = 1;
  endspecify
endmodule
module tb;
  reg a;
  wire y;
  m u(.a(a), .y(y));
  initial #1 $finish;
endmodule
