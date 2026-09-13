`timescale 10ns/100ps
module slow(input source, output result);
    assign #0.25 result = source;
endmodule
`timescale 1ns/100ps
module tb;
    logic source;
    wire fast_result, slow_result;
    assign #0.25 fast_result = source;
    slow child(source, slow_result);
    initial begin
        source = 0;
        #0.1 source = 1;
        #0.2 $strobe("0.3 %b %b", fast_result, slow_result);
        #0.1 $strobe("0.4 %b %b", fast_result, slow_result);
        #2.2 $strobe("2.6 %b %b", fast_result, slow_result);
        #1 $finish(0);
    end
endmodule
