// IEEE 1364-2001 has no operator overload declarations (and no 'bind').
module tb;
  function [7:0] add;
    input [7:0] a, b;
    add = a + b;
  endfunction
  bind + function reg [7:0] add(reg [7:0], reg [7:0]);
endmodule
