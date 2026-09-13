`timescale 1fs/1fs
module tb;
    reg value;
    initial begin
        $dumpfile("trace.vcd");
        value = 1'b0;
        $dumpvars;
        #10 value = 1'b1;
        if (!value) $finish(1);
        $dumpflush;
        #1 $finish(0);
    end
endmodule
