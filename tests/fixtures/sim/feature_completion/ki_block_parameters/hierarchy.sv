`timescale 1ns/1ps
module tb;
  int z;
  initial begin : b
    localparam K = 2;
    $display("%0d", K);
    begin : c localparam K = 30; z = tb.b.K + b.K + K; $display("%0d", z); end
  end
  initial #1 $display("%0d %0d", b.K, tb.b.c.K);
endmodule
