// R04: the same typedef avoids an incidental cast hiding the module RHS gate.
module tb;
    typedef logic [7:0] array_t [0:1];
    array_t a, b, result;
    logic [1:0] selector;
    always_comb result = selector ? a : b;

    initial begin
        a = '{8'ha5, 8'h3c};
        b = '{8'ha6, 8'h3c};
        selector = 0;
        #1;
        $display("false=%h,%h", result[0], result[1]);
        selector = 1;
        #1;
        $display("true=%h,%h", result[0], result[1]);
        selector = 2'b0x;
        #1;
        $display("unknown=%h,%h", result[0], result[1]);
        selector = 2'b0z;
        #1;
        $display("highz=%h,%h", result[0], result[1]);
        b[0] = 8'ha5;
        #1;
        $display("equal=%h,%h", result[0], result[1]);
        a[1] = 8'hf0;
        #1;
        $display("changed=%h,%h", result[0], result[1]);
        selector = 2'bx1;
        #1;
        $display("known_one=%h,%h", result[0], result[1]);
        $finish(0);
    end
endmodule
