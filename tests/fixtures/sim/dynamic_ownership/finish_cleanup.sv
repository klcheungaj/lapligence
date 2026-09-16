`timescale 1ns/1ns
module tb;
    logic [65536:0] scratch;
    initial begin
        scratch = '0;
        forever begin
            scratch[7:0] = scratch[7:0] + 8'd1;
            #1;
        end
    end
    initial begin
        #8;
        $display("finish");
        $finish;
    end
endmodule
