// IEEE 1800-2009 §§6.22.1 and 10.9.2: the nonmatching type key leaves the
// structure member uncovered when no default is supplied.
module tb;
    typedef struct packed { logic [7:0] value; } left_lane_t;
    typedef struct packed { logic [7:0] value; } right_lane_t;
    typedef struct packed { left_lane_t lane; } holder_t;

    holder_t value = '{right_lane_t: 8'hff};
endmodule
