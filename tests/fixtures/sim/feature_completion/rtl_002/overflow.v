// Legal syntax; IEEE minimum capacity does not require this overflowing u64 product.
module tb;
    reg memory [-2147483648:2147483647][-2147483648:2147483647][-2147483648:2147483647];
    initial $finish(0);
endmodule
