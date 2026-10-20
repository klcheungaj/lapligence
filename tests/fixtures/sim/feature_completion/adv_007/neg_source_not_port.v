// A path source is an input or inout port (the frontend reports it).
module m(input a, output y);
  wire w = a;
  assign y = w;
  specify
    (w => y) = 1;
  endspecify
endmodule
module tb;
  reg a;
  wire y;
  m u(.a(a), .y(y));
  initial #1 $finish;
endmodule
