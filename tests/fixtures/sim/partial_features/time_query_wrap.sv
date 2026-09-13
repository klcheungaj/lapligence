`timescale 1ns/1ns
module tb;
    initial begin
        #(64'd4294967298);
        $display("large time=%0d stime=%0d realtime=%.0f", $time, $stime, $realtime);
        $finish(0);
    end
endmodule
