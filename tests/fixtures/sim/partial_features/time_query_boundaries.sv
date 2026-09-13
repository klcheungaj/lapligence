`timescale 10ns/1ps
module tb;
    initial begin
        #1.49;
        $display("below time=%0d stime=%0d realtime=%.2f", $time, $stime, $realtime);
        #0.01;
        $display("half time=%0d stime=%0d realtime=%.2f", $time, $stime, $realtime);
        #0.01;
        $display("above time=%0d stime=%0d realtime=%.2f", $time, $stime, $realtime);
        $finish(0);
    end
endmodule
