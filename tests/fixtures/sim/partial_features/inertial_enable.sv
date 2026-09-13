`timescale 1ns/1ns
module tb;
    logic source, enable;
    wire result;
    bufif1 #2 g(result, source, enable);
    initial begin
        source = 1; enable = 0;
        #2 $strobe("disabled %b", result);
        #1 enable = 1;
        #1 enable = 0;
        #3 $strobe("canceled %b", result);
        source = 0; enable = 1'bx;
        #2 $strobe("unknown %b", result);
        #1 enable = 1;
        #2 $strobe("enabled %b", result);
        #1 $finish(0);
    end
endmodule
