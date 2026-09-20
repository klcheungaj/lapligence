module tb;
    typedef logic [7:0] array_t [0:1];
    array_t a, b, result;
    logic clk, selector;
    always_ff @(posedge clk) result <= selector ? a : b;
    initial begin
        clk = 0; selector = 0;
        a = '{8'ha5, 8'h5a}; b = '{8'ha6, 8'h5a};
        #1; clk = 1;
        #1;
        $display("clock0=%h,%h", result[0], result[1]);
        clk = 0; selector = 1;
        #1; clk = 1;
        #1;
        $display("clock1=%h,%h", result[0], result[1]);
        clk = 0; selector = 1'bx;
        #1; clk = 1;
        #1;
        $display("clockx=%h,%h", result[0], result[1]);
        $finish(0);
    end
endmodule
