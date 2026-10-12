// SIM-040 negative: an unpacked aggregate above the payload width limit is rejected.
module tb;
    import "DPI-C" function void neg_big(input int a [40000]);
    int a [40000];
    initial neg_big(a);
endmodule
