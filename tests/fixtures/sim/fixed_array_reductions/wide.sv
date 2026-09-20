module tb;
    logic [64:0] a [0:1];
    logic [128:0] b [-1:-2];
    initial begin
        a[0] = '1; a[1] = 2;
        if (a.sum() !== 65'd1) $fatal(1, "65-bit sum");
        if (a.product() !== 65'h1fffffffffffffffe) $fatal(1, "65-bit product");
        if (a.and() !== 65'd2) $fatal(1, "65-bit and");
        if (a.or() !== 65'h1ffffffffffffffff) $fatal(1, "65-bit or");
        if (a.xor() !== 65'h1fffffffffffffffd) $fatal(1, "65-bit xor");
        b[-1] = '1; b[-2] = 2;
        if (b.sum() !== 129'd1) $fatal(1, "129-bit sum");
        if (b.product() !== {128'hffffffffffffffffffffffffffffffff, 1'b0})
            $fatal(1, "129-bit product");
        if (b.and() !== 129'd2) $fatal(1, "129-bit and");
        if (b.or() !== {129{1'b1}}) $fatal(1, "129-bit or");
        if (b.xor() !== ({129{1'b1}} ^ 129'd2)) $fatal(1, "129-bit xor");
        if ((a.sum() with (129'(item))) !== 129'h20000000000000001)
            $fatal(1, "widen before accumulation");
        $display("wide=ok");
        $finish(0);
    end
endmodule
