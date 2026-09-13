`timescale 1ns/1ps
module tb;
  real q, source;
  initial begin
    source = 1.0;
    q = 0.0;
    assign q = source;
    q = 0.0;
    #0 $display("CHECK: real_blocking=%.1f", q);
    q <= 0.0;
    #1 $display("CHECK: real_nba=%.1f", q);
    source = 2.0;
    #1 $display("CHECK: real_live=%.1f", q);
    force q = 9.0;
    source = 3.0;
    #1 $display("CHECK: real_forced=%.1f", q);
    release q;
    #0 $display("CHECK: real_released=%.1f", q);
    deassign q;
    q = 4.0;
    $display("CHECK: real_deassign=%.1f", q);
    $finish(0);
  end
endmodule
