module tb;
    typedef logic [7:0] array_t [0:1];
    typedef bit [7:0] bits_t [0:1];
    array_t a, b, result;
    bits_t ba, bb, bit_result;
    logic p, q;
    always_comb begin
        result = p &&& q ? a : b;
        bit_result = p &&& q ? ba : bb;
    end
    initial begin
        a = '{8'ha5, 8'h5a}; b = '{8'ha6, 8'h5a};
        ba = '{8'ha5, 8'h5a}; bb = '{8'ha6, 8'h5a};
        p = 1; q = 1; #1;
        if (result[0] !== 8'ha5 || bit_result[0] !== 8'ha5) $fatal(1, "true array");
        q = 0; #1;
        if (result[0] !== 8'ha6) $fatal(1, "false array");
        p = 1'bx; #1;
        if (result[0] !== 8'hxx || result[1] !== 8'h5a ||
            bit_result[0] !== 8'h00 || bit_result[1] !== 8'h5a)
            $fatal(1, "aggregate default boundary after early X");
        b[0] = 8'ha5; #1;
        if (result[0] !== 8'ha5) $fatal(1, "equal aggregate element");
        p = 0; q = 1'bz; #1;
        if (result[0] !== 8'ha5 || bit_result[0] !== 8'ha6) $fatal(1, "array short circuit");
        $display("array_results=pass");
        $finish(0);
    end
endmodule
