`timescale 1ns/1ns
module tb;
    wire [7:0] result;
    assign #7 result = 8'h2a;
    final $display("final %0d %h", $time, result);
endmodule
