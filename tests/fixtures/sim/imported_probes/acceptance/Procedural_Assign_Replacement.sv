`timescale 1ns/1ps
module tb;
  reg q, a, b;
  initial begin
    a = 0; b = 1;
    assign q = a;
    #1 $display("CHECK: first=%b", q);
    assign q = b;
    #1 $display("CHECK: second=%b", q);
    b = 0;
    #1 $display("CHECK: follows=%b", q);
    deassign q;
    $finish(0);
  end
endmodule
