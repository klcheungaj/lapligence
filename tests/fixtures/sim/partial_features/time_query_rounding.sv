`timescale 10ns/1ns
module slow;
    initial begin
        #1.6;
        $display("slow time=%0d stime=%0d realtime=%.3f", $time, $stime, $realtime);
    end
endmodule

`timescale 1ns/1ps
module fast;
    initial begin
        #16;
        $display("fast time=%0d stime=%0d realtime=%.3f", $time, $stime, $realtime);
    end
endmodule

module tb;
    slow u_slow();
    fast u_fast();
    initial begin
        #17;
        $finish(0);
    end
endmodule
