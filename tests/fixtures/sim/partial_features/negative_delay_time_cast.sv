`timescale 1ps/1ps
module tb;
    logic signed [7:0] delay_value=-2;
    initial begin
        #delay_value $display("%0d",$time);
        $finish(0);
    end
endmodule
