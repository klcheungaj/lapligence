// SIM-040 negative: an unpacked union is not a legal DPI-C formal type (H.7.8).
module tb;
    typedef union { int a; int b; } u_t;
    import "DPI-C" function void neg_union(input u_t a);
    u_t u;
    initial neg_union(u);
endmodule
