`timescale 1ns/1ps
module tb;
  initial begin
    $display("CHECK: before");
    $finish(0);
    $display("CHECK: after");
  end
  final $display("CHECK: final");
endmodule
