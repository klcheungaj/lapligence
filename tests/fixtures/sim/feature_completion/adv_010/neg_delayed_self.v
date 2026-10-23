// A delayed signal cannot be the signal it delays.
`timescale 1ns/1ns
module m(input c, input d);
  reg n;
  wire dd;
  specify
    $setuphold(posedge c, d, -2, 5, n, , , c, dd);
  endspecify
endmodule
module tb;
  reg c, d;
  m u(c, d);
  initial begin $display("ran"); #1 $finish; end
endmodule
