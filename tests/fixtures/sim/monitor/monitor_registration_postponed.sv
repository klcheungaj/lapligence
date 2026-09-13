`timescale 1ns/1ps
module tb;
  integer q;
  initial begin
    q = 0;
    $monitor("CHECK: q=%0d", q);
    q = 1;
    q <= 2;
    #1 $monitoroff;
    $finish(0);
  end
endmodule
