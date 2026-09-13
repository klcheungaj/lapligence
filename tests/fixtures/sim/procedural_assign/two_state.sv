`timescale 1ns/1ps
module tb;
  bit q;
  logic source;
  initial begin
    source = 1'bx;
    assign q = source;
    #1 $display("CHECK: unknown=%b", q);
    source = 1'b1;
    #1 $display("CHECK: known=%b", q);
    deassign q;
    $finish(0);
  end
endmodule
