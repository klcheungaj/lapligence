// Both shapes occupy 32 bits, but flattening does not make their ranks compatible.
module tb;
    typedef logic [7:0] matrix_t [0:1][0:1];
    typedef logic [7:0] flat_t [0:3];
    matrix_t a, b;
    flat_t result;
    logic selector;
    initial begin
        selector = 1;
        result = selector ? a : b;
    end
endmodule
