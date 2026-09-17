`timescale 1ns/1ps
module tb;
  reg q, d;
  initial begin
    d = 1; q = 0;
    assign q = d;
    q = 0;
    #0 $display("CHECK: blocking=%b", q);
    q <= 0;
    #1 $display("CHECK: nba=%b", q);
    deassign q;
    q = 0;
    $display("CHECK: deassign=%b", q);
    $finish(0);
  end
endmodule
