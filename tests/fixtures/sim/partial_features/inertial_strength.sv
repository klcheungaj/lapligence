`timescale 1ns/1ns
module tb;
    logic strong_source, weak_source;
    wire result;
    assign (strong1, strong0) #2 result = strong_source;
    assign (weak1, weak0) #4 result = weak_source;
    initial begin
        strong_source = 0; weak_source = 1;
        #2 $strobe("strong %b", result);
        #2 $strobe("conflict %b", result);
        #1 strong_source = 1'bz;
        #2 $strobe("released %b", result);
        #1 strong_source = 0;
        #1 strong_source = 1'bz;
        #3 $strobe("canceled %b", result);
        #1 $finish(0);
    end
endmodule
