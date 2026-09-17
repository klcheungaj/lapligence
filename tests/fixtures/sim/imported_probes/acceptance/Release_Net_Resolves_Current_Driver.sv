`timescale 1ns/1ps
module tb;
  reg d;
  wire w;
  assign w = d;
  initial begin
    d = 0;
    #1 force w = 0;
    d = 1;
    #1 release w;
    $display("CHECK: resolved=%b", w);
    $finish(0);
  end
endmodule
