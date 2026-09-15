`timescale 1ns/100ps
module tb;
    parameter real P = 0.25;
    initial begin : local_scope
        real P;
        P = 1.5;
        #P;
        $display("DELAY=%0.1f", $realtime);
        $finish(0);
    end
endmodule
