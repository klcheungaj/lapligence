// A continuous assignment passes an element of a queue to a function (6.21).
`timescale 1ns/1ps
module tb;
  int pq [$] = '{1, 2, 3};
  int x;
  function automatic int f(int v);
    return v + 1;
  endfunction
  assign x = f(pq[1]);
  initial #1 $display("x=%0d", x);
endmodule
