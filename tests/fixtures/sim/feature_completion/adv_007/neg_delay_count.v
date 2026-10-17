// A path delay has 1, 2, 3, 6 or 12 values (the frontend reports it).
module m(input a, output y);
  assign y = a;
  specify
    (a => y) = (1, 2, 3, 4);
  endspecify
endmodule
module tb;
  reg a;
  wire y;
  m u(.a(a), .y(y));
  initial #1 $finish;
endmodule
