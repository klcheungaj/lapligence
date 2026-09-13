`timescale 10ns/1ps
module child;
    initial begin #0.025 $display("child %.3f",$realtime); end
endmodule
`timescale 1ns/1ps
module tb;
    child u();
    real t;
    initial begin
        #0.125 t=$realtime;
        $display("parent %.3f",t);
        #0.25 $display("parent %.3f",$realtime);
        $finish(0);
    end
endmodule
