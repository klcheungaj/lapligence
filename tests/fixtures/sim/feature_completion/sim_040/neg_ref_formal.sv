// SIM-040 negative: 35.5.6 forbids ref formals in imports.
module tb;
    import "DPI-C" function void neg_ref(ref int a);
    int x;
    initial neg_ref(x);
endmodule
