module tb;
    typedef logic [64:0] lane_t;
    lane_t values[-1:1];
    lane_t seed;
    localparam int KEY = 0;
    function automatic int center_key();
        return KEY;
    endfunction
    initial begin
        seed = '1;
        values = '{(KEY):seed, (center_key()):seed, lane_t:seed};
    end
endmodule
