// SIM-040 negative: two imports of one C name need identical type signatures (35.5.4).
module m1;
    import "DPI-C" function int neg_twice(input int a);
    initial $display("%0d", neg_twice(3));
endmodule

module m2;
    import "DPI-C" function int neg_twice(input shortint b);
    initial $display("%0d", neg_twice(4));
endmodule

module tb;
    m1 u1();
    m2 u2();
endmodule
