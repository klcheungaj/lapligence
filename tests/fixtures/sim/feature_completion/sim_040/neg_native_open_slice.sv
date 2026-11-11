// SIM-040 negative: an open-array actual with real, shortreal, string or
// chandle elements crosses in its own storage, so it is a whole array
// variable, not a slice.
module tb;
    import "DPI-C" function real neg_sum(input real a []);
    real values [4];
    initial $display("%g", neg_sum(values[0:1]));
endmodule
