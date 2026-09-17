`timescale 1ns/1ns
module tb;
    logic [128:0] target;
    logic [64:0] source;
    initial begin
        target = '0;
        source = 65'd42;
        target[64 +: 65] <= #2 source;
        source = 65'd99;
        #1;
        $display("%0d", target[64 +: 65]);
        #2;
        $display("%0d", target[64 +: 65]);
        $finish(0);
    end
endmodule
