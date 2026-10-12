// SIM-040 negative: 35.5.5 restricts function results to small values; a packed
// vector result is illegal.
module tb;
    import "DPI-C" function bit [15:0] neg_result();
    initial $display("%h", neg_result());
endmodule
