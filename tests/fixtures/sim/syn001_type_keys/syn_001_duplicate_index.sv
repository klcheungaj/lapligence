// IEEE 1800-2009 10.9.1-10.9.2: an explicit fixed-array index key may occur
// only once in an assignment pattern.
module tb;
    typedef logic [7:0] lane_t;
    lane_t value[0:1] = '{0: 8'h11, 0: 8'h22};
    initial $display("UNEXPECTED syn_001_duplicate_index");
endmodule
