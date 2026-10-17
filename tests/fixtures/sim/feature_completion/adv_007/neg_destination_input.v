// A path destination is an output or inout port (the frontend reports it).
module m(input a, input b, output y);
  assign y = a;
  specify
    (a => b) = 1;
  endspecify
endmodule
module tb;
  reg a, b;
  wire y;
  m u(.a(a), .b(b), .y(y));
  initial #1 $finish;
endmodule
