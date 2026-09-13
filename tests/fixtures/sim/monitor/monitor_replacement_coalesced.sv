`timescale 1ns/1ps
module tb;
  integer q;
  initial begin
    q = 0;
    $monitor("OLD: q=%0d", q);
    $monitor("NEW: q=%0d", q);
    q = 1;
    q = 2;
    q <= 3;
    #1 $finish(0);
  end
endmodule
