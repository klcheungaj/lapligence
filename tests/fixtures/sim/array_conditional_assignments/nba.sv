module tb;
    typedef logic [7:0] array_t [0:1];
    array_t a, b, result;
    logic [7:0] rows [1:0][0:1];
    logic selector;
    int target;
    initial begin
        a = '{8'h11, 8'h22}; b = '{8'h33, 8'h44};
        result = '{default: 8'h00}; rows = '{default: '{default: 8'h00}};
        selector = 1;
        result <= selector ? a : b;
        a = '{8'h55, 8'h66}; selector = 0;
        if (result[0] !== 0 || result[1] !== 0) $fatal(1, "NBA committed early");
        #1;
        if (result[0] !== 8'h11 || result[1] !== 8'h22) $fatal(1, "NBA source was not captured");
        result <= selector ? a : b;
        result[1] <= 8'hee;
        b = '{8'h77, 8'h88};
        #1;
        if (result[0] !== 8'h33 || result[1] !== 8'hee) $fatal(1, "NBA ordering");
        a = '{8'ha5, 8'h5a}; b = '{8'ha6, 8'h5a}; selector = 1'bx;
        target = 1;
        rows[target] <= selector ? a : b;
        target = 0; selector = 0; a = '{default: 8'h00}; b = a;
        #1;
        if (rows[1][0] !== 8'hxx || rows[1][1] !== 8'h5a) $fatal(1, "NBA array mux capture");
        if (rows[0][0] !== 0 || rows[0][1] !== 0) $fatal(1, "NBA target index was not captured");
        $display("nba=%h,%h row=%h,%h", result[0], result[1], rows[1][0], rows[1][1]);
        $finish(0);
    end
endmodule
