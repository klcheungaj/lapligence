`timescale 1ns/1ps
module tb;
  reg q, source;
  initial begin
    source = 1;
    assign q = source;
    #1 force q = 0;
    #1 source = 0;
    #1 $display("CHECK: forced=%b", q);
    release q;
    $display("CHECK: released=%b", q);
    deassign q;
    $finish(0);
  end
endmodule
