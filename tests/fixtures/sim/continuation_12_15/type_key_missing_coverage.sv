module tb;
    typedef logic [128:0] lane_t;
    lane_t values[-1:1];
    lane_t seed;
    initial begin
        seed = '1;
        values = '{0:seed, int:17};
    end
endmodule
