// IEEE 1800-2009 10.9.1-10.9.2: a fixed-array assignment pattern must cover
// every declared index unless a matching type key or default supplies it.
module tb;
    typedef logic [7:0] lane_t;
    lane_t value[0:1] = '{0: 8'h11};
    initial $display("UNEXPECTED syn_001_missing_coverage");
endmodule
