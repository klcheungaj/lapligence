`timescale 1ns/1ns
module tb;
    logic [128:0] source;
    wire [128:0] result;
    assign #3 result = source;
    initial begin
        source = '0;
        #1 source[128] = 1;
        #1 source[0] = 1;
        #1 $strobe("3 %b %b", result[128], result[0]);
        #1 $strobe("4 %b %b", result[128], result[0]);
        #1 $strobe("5 %b %b", result[128], result[0]);
        #1 source[65:62] = 4'b1xz0;
        #3 $strobe("states %b %b %b", result[128], result[65:62], result[0]);
        #1 $finish;
    end
endmodule
