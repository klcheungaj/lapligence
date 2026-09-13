`timescale 1ns/1ns
module tb;
    logic a, b;
    wire result;
    assign #2 result = a;
    assign #4 result = b;
    initial begin
        a = 0; b = 0;
        #2 $strobe("2 %b", result);
        #2 $strobe("4 %b", result);
        #1 a = 1;
        #1 a = 0;
        #2 $strobe("8 %b", result);
        a = 1; b = 1;
        #2 $strobe("10 %b", result);
        #2 $strobe("12 %b", result);
        #1 $finish(0);
    end
endmodule
