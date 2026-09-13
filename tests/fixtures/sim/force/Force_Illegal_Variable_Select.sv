// IEEE 1800-2009 §10.6.2: this simulator admits only constant-selected net
// force targets; a variable bit-select has no canonical bounded overlay.
`timescale 1ns/1ps
module tb;
  reg [3:0] value;
  integer index;

  initial begin
    value = 4'h0;
    index = 1;
    force value[index] = 1'b1;
    $finish(0);
  end
endmodule
