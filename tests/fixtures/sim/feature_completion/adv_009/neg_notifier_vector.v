// A notifier must be a scalar variable (31.6); the valid check beside it is
// not simulated either.
`timescale 1ns/1ns
module m(input d, input c);
  reg ok;
  reg [1:0] n;
  specify
    $hold(posedge c, d, 2, ok);
    $setup(d, posedge c, 10, n);
  endspecify
endmodule
module tb;
  reg c, d;
  m u(d, c);
  initial begin $display("ran"); #1 $finish; end
endmodule
