// An integral delay wider than 64 bits keeps its low 64 bits, like a time
// variable (decision S01-D1): 2^100 is a zero delay.
`timescale 1ps/1ps
module tb;
    logic [128:0] delay_value;
    initial begin
        delay_value=129'd1<<100;
        #delay_value;
        $display("after %0t", $realtime);
        $finish(0);
    end
endmodule
