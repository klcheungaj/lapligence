`timescale 1ns/1ps
module tb;
  integer q;
  initial begin
    q = 9;
    $monitor("CHECK: q=%0d", q);
    #1 $monitoroff;
    #1 $monitoron;
    #1 $finish(0);
  end
endmodule
