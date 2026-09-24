// An unmatched type key without a default leaves this array uncovered.
module tb;
    typedef logic [7:0] lane_t;
    typedef logic signed [7:0] signed_lane_t;
    lane_t value[0:1] = '{signed_lane_t: 8'h11};
    initial $display("UNEXPECTED syn_001_incompatible_type_key");
endmodule
