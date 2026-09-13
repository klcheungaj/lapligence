`timescale 1ns/1ns
module tb;
    reg source;
    wire result;
    not #3 g(result, source);
    initial begin
        source = 0;
        #1 source = 1;
        #1 source = 0;
        #1 $strobe("3 %b", result);
        #1 source = 1;
        #2 $strobe("6 %b", result);
        #1 $strobe("7 %b", result);
        #1 $finish(0);
    end
endmodule
