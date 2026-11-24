`timescale 1ns/1ps
module sub #(parameter W = 3) ();
  initial begin : b
    parameter P = W * 2; typedef logic [W-1:0] T;
    T t; t = '1;
    $display("%m %0d %0d", P, t);
  end
  function automatic int f(int x); parameter K = W; begin : q localparam J = K + W; return x + K + J; end endfunction
  initial #1 $display("%0d", f(1));
endmodule
module tb; sub s1(); sub #(5) s2(); initial #2 $display("%0d %0d", s1.b.P, s2.b.P); endmodule
