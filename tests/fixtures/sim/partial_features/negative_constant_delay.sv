`timescale 1ps/1ps
module tb;
    parameter integer DELAY=-2;
    initial begin
        #DELAY $display("%0d",$time);
        $finish;
    end
endmodule
