// SIM-040 negative: open arrays of real elements are rejected.
module tb;
    import "DPI-C" function void neg_reals(input real a []);
    real a [2];
    initial neg_reals(a);
endmodule
