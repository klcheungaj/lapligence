// Two declarations of one path are an error (decision A7-D4).
module m(input a, output y);
  assign y = a;
  specify
    (a => y) = 1;
    (a => y) = 2;
  endspecify
endmodule
module tb;
  reg a;
  wire y;
  m u(.a(a), .y(y));
  initial #1 $finish;
endmodule
