// IEEE 1800-2009 §21.7: loop-based registration preserves escaped array names.
module tb;
    logic [7:0] \memory%odd [0:4096];
    initial begin
        $dumpfile("fixed.vcd");
        $dumpvars(0, tb);
        #1;
        \memory%odd [4096] = 8'h5a;
        #1;
        if (\memory%odd [0] !== 8'bx || \memory%odd [4096] !== 8'h5a) $fatal;
        $display("PASS rtl002 waveform");
        $finish(0);
    end
endmodule
