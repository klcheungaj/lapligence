module tb;
    typedef logic [7:0] lane_t;
    typedef struct {
        lane_t a;
        lane_t b;
    } pair_t;

    pair_t bad = '{a: 8'h1, a: 8'h2, b: 8'h3};
    initial begin
        $display("PASS pattern_bad_keys");
    end
endmodule
