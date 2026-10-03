// IEEE 1800-2009 Table 10-1: continuous assignment targets admit only
// constant selects, including positional pattern leaves.
module tb;
    typedef logic [1:0][3:0] pair_t;
    logic [3:0] v[0:1];
    logic [3:0] m;
    logic i = 0;
    pair_t x = 8'h12;
    assign '{v[i], m} = x;
endmodule
