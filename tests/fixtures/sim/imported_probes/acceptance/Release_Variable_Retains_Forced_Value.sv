`timescale 1ns/1ps
module tb;
  reg q;
  initial begin
    q = 0;
    force q = 1;
    #1 release q;
    $display("CHECK: retained=%b", q);
    #1 q = 0;
    $display("CHECK: next_write=%b", q);
    $finish(0);
  end
endmodule
