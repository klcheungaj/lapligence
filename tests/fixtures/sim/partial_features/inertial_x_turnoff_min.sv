// Unknown transitions use the minimum of rise, fall, and turn-off delays.
`timescale 1ns/1ps
module tb;
    logic source, enable;
    wire continuous_result, gate_result;
    wire #(5, 7, 1) declaration_result = source;
    assign #(5, 7, 1) continuous_result = source;
    bufif1 #(5, 7, 1) gate_driver(gate_result, source, enable);

    initial begin
        source = 0;
        enable = 1;
        #8;
        source = 1'bx;
        #2 $display("x_min %b %b %b", continuous_result, gate_result,
                    declaration_result);
        $finish(0);
    end
endmodule
