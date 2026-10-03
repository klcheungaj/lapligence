module tb;
    logic [128:0] a, shifted;
    initial begin
        a = 17;
        #1;
        shifted = a << 1;
        $display("shift=%0d", shifted);
        $finish(0);
    end
endmodule
