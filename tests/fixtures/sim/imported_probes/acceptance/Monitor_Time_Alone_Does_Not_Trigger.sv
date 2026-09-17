`timescale 1ns/1ps
module tb;
  integer q;
  initial begin
    q = 7;
    $monitor("CHECK: time=%0d q=%0d", $time, q);
    #1;
    #1;
    $monitoroff;
    $finish(0);
  end
endmodule
