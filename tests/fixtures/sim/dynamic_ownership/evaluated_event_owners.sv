module tb;
    logic [7:0] a, b;
    bit enabled;
    initial begin
        a = 0;
        b = 0;
        enabled = 0;
        #1 a = 1;
        #1 enabled = 1;
        #1 b = 1;
        #1 $finish(0);
    end
    initial begin
        @(a + b iff enabled);
        $display("qualified");
    end
endmodule
