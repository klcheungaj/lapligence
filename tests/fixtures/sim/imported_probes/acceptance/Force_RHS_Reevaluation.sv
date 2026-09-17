`timescale 1ns/1ps
module tb;
  reg q, d;
  initial begin
    q = 0; d = 1;
    force q = d;
    #1 $display("CHECK: initial=%b", q);
    d = 0;
    #1 $display("CHECK: follows=%b", q);
    release q;
    $finish(0);
  end
endmodule
