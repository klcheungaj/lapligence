// State-dependent paths are ADV-008: rejected with their location.
module m(input a, input s, output y);
  assign y = a;
  specify
    if (s) (a => y) = 1;
  endspecify
endmodule
module tb;
  reg a, s;
  wire y;
  m u(.a(a), .s(s), .y(y));
  initial #1 $finish;
endmodule
