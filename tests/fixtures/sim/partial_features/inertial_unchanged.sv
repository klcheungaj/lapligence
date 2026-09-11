`timescale 1ns/1ns
module tb;
    logic a, b;
    wire result, gate_result;
    assign #3 result = a & b;
    and #3 g(gate_result, a, b);
    initial begin
        a = 0; b = 0;
        #1 b = 1;
        #2 $strobe("stable %b %b", result, gate_result);
        #1 a = 1;
        #1 a = 0;
        #3 $strobe("canceled %b %b", result, gate_result);
        a = 1;
        #3 $strobe("propagated %b %b", result, gate_result);
        #1 $finish;
    end
endmodule
