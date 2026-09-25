module tb;
    typedef logic [6:0] lane_t;
    lane_t values[2];
    int unpacked_value[2];
    initial begin
        unpacked_value = '{1, 2};
        values = '{lane_t:unpacked_value};
    end
endmodule
