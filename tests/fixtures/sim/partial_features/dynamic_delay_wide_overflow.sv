`timescale 1ps/1ps
module tb;
    logic [128:0] delay_value;
    initial begin
        delay_value=129'd1<<100;
        #delay_value;
        $finish;
    end
endmodule
