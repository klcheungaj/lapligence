module tb;
  function [3:0] inc;
    input [3:0] v;
    inc = v + 1;
  endfunction
  reg [3:0] r;
  initial r = inc(.v(4'd2));
endmodule
