`timescale 1ns/100ps
module tb;
    parameter real P = 0.25;
    task t(input integer P);
        #P;
        $display("task %0d", $time);
    endtask
    initial begin
        t(1);
        t(2);
        $finish(0);
    end
endmodule
