`timescale 1ps/1ps
module tb;
    reg source;
    wire result;
    assign #(64'hffffffffffffffff) result = source;
    initial begin
        #1 source = 1;
        #2 $finish(0);
    end
endmodule
