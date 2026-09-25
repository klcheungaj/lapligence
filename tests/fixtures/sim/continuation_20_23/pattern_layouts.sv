// llg-test-fixture: SYN-003 exact-width, signed/state and packed-array deconstruction.
module lanes #(parameter W = 7)(output bit done);
    typedef logic signed [W-1:0] lane_t;
    typedef bit [W-1:0] bits_t;
    typedef lane_t reversed_t[2:1];
    typedef logic [1:0][W-1:0] packed_t;
    typedef struct { lane_t signed_lane; bits_t known_lane; } mixed_t;
    lane_t first, second;
    bits_t converted;
    reversed_t value;
    packed_t packed_value;
    mixed_t mixed;
    initial begin
        done = 0;
        value[2] = lane_t'(-1);
        value[1] = 'z;
        '{first, second} = value;
        if (first !== lane_t'(-1) || second !== {W{1'bz}}) $fatal(1, "reversed lanes");
        mixed = '{lane_t'(-1), '1};
        mixed_t'{first, converted} = mixed;
        if (first !== lane_t'(-1) || converted !== {W{1'b1}}) $fatal(1, "mixed record");
        packed_value = {{W{1'bx}}, {W{1'bz}}};
        packed_t'{converted, second} = packed_value;
        if (converted !== '0 || second !== {W{1'bz}}) $fatal(1, "packed target state");
        done = 1;
    end
endmodule
module tb;
    wire [3:0] done;
    lanes #(1) a(done[0]);
    lanes #(7) b(done[1]);
    lanes #(65) c(done[2]);
    lanes #(129) d(done[3]);
    initial begin
        #1;
        if (done !== 4'hf) $fatal(1, "unfinished widths");
        $display("PATTERN_LAYOUTS_PASS");
        $finish(0);
    end
endmodule
