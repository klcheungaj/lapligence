// IEEE 1800-2009 10.9.1-10.9.2: a fixed-array type key must match the
// declared element type, including its signedness and state domain.
module tb;
    typedef logic [7:0] lane_t;
    typedef logic signed [7:0] signed_lane_t;
    lane_t value[0:1] = '{signed_lane_t: 8'h11, default: 8'h22};
    initial $display("UNEXPECTED syn_001_incompatible_type_key");
endmodule
