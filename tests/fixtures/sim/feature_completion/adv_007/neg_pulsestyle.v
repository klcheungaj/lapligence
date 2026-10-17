// pulsestyle_onevent is ADV-008: rejected with its location.
module m(input a, output y);
  assign y = a;
  specify
    pulsestyle_onevent y;
    (a => y) = 3;
  endspecify
endmodule
module tb;
  reg a;
  wire y;
  m u(.a(a), .y(y));
  initial #1 $finish;
endmodule
