// Equal array lengths do not make differently sized element types equivalent.
module tb;
    typedef logic [7:0] narrow_t [0:1];
    typedef logic [15:0] wide_t [0:1];
    narrow_t a, b;
    wide_t result;
    logic selector;
    initial begin
        selector = 1;
        result = selector ? a : b;
    end
endmodule
