// IEEE 1800-2009 10.9.2: one assignment-pattern index may occur only once.
module tb;
    typedef logic [7:0] lane_t;
    typedef struct {
        lane_t bytes[0:1];
    } aggregate_t;

    aggregate_t value;
    initial value = '{bytes: '{0: 8'h11, 0: 8'h22}};
endmodule
