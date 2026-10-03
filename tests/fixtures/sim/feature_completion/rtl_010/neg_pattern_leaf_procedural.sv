// IEEE 1800-2009 6.5 and 10.9: a positional pattern leaf is a continuous
// writer of that cell.
module tb;
    typedef logic [1:0][3:0] pair_t;
    logic [3:0] arr[0:2];
    pair_t x = 8'h12;
    assign '{arr[0], arr[1]} = x;
    initial arr[1] = 4'h2;
endmodule
