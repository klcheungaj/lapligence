// The event_based and remain_active flags need known constant values.
`timescale 1ns/1ns
module m(input a, input b);
  reg n;
  specify
    $timeskew(posedge a, negedge b, 5, n, 1'bx);
  endspecify
endmodule
module tb;
  reg a, b;
  m u(a, b);
  initial begin $display("ran"); #1 $finish; end
endmodule
