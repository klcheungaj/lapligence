// IEEE 1800-2009 10.9.2: an assignment-pattern index must be in the
// declared fixed-array range.
module tb;
    typedef logic [7:0] lane_t;
    typedef struct {
        lane_t bytes[0:1];
    } aggregate_t;

    aggregate_t value;
    initial value = '{bytes: '{2: 8'h11, default: 8'h22}};
endmodule
