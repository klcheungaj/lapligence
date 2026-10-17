// An inout destination also driven outside the module would delay that
// driver too; rejected as not supported.
module m(input a, inout y);
  assign y = a;
  specify
    (a => y) = 3;
  endspecify
endmodule
module tb;
  reg a, e;
  wire y;
  m u(.a(a), .y(y));
  assign y = e ? 1'b0 : 1'bz;
  initial #1 $finish;
endmodule
