`timescale 1ns/1ps
module tb;
  integer q;
  initial begin
    q = 0;
    $monitor("CHECK: q=%0d", q);
    #1 begin
      $monitoroff;
      q = 1;
      q = 2;
      q <= 4;
      $monitoron;
    end
    #1 $finish(0);
  end
endmodule
