`timescale 1ns/1ps
module tb;
    logic [63:0] delay_value;
    initial begin
        delay_value=64'hffffffffffffffff;
        #delay_value;
        $finish(0);
    end
endmodule
