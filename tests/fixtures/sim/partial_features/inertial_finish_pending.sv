`timescale 1ns/1ns
module tb;
    logic source = 0;
    wire result;
    assign #1000000 result = source;
    initial begin
        #1 source = 1;
        #1 source = 0;
        #1 $display("pending %b", result);
        $finish;
    end
    final $display("final %0d", $time);
endmodule
