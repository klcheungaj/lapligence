// IEEE 1800-2009 §§6.22.1 and 10.9.2: a nonmatching type key does not set a
// member, so that member receives the assignment pattern's typed default.
module tb;
    typedef struct packed { logic [7:0] value; } left_lane_t;
    typedef struct packed { logic [7:0] value; } right_lane_t;
    typedef struct packed { left_lane_t lane; } holder_t;

    holder_t value = '{
        right_lane_t: 8'hff,
        default: left_lane_t'('0)
    };

    initial begin
        if (value.lane.value !== 8'h00) begin
            $fatal(1, "packed nominal type key default mismatch: %h", value.lane.value);
        end else begin
            $display("PASS packed_nominal_type_key_default");
        end
        $finish;
    end
endmodule
