// IEEE 1800-2009 6.24.3, 7.2-7.3: fixed aggregate bit-stream probe.
module tb;
    typedef logic [7:0] word_t;
    typedef logic [3:0] lane_t;
    typedef lane_t lane_array_t [0:1];
    typedef lane_t lane_desc_array_t [1:0];
    typedef bit [3:0] bit_lane_t;
    typedef bit_lane_t bit_lane_array_t [0:1];
    typedef struct packed { logic [3:0] hi; logic [3:0] lo; } packed_t;
    typedef struct { logic [3:0] hi; logic [3:0] lo; } unpacked_t;
    lane_t lanes [0:1];
    lane_desc_array_t desc_lanes;
    packed_t packed_value;
    packed_t packed_from_unpacked;
    unpacked_t unpacked_value;
    unpacked_t unpacked_from_word;
    word_t from_lanes;
    word_t from_packed;
    word_t from_unpacked;
    lane_array_t lanes_from_word;
    lane_desc_array_t desc_from_word;
    bit_lane_array_t bits_from_xz;
    word_t from_desc;
    word_t packed_input;
    initial begin
        lanes[0] = 4'ha;
        lanes[1] = 4'hb;
        desc_lanes[1] = 4'ha;
        desc_lanes[0] = 4'hb;
        packed_value = '{4'hc, 4'hd};
        unpacked_value = '{4'he, 4'hf};
        from_lanes = word_t'(lanes);
        from_desc = word_t'(desc_lanes);
        from_packed = word_t'(packed_value);
        from_unpacked = word_t'(unpacked_value);
        packed_from_unpacked = packed_t'(unpacked_value);
        packed_input = 8'hcd;
        lanes_from_word = lane_array_t'(packed_input);
        desc_from_word = lane_desc_array_t'(packed_input);
        bits_from_xz = bit_lane_array_t'(8'b1x0z_zz10);
        unpacked_from_word = unpacked_t'(packed_input);
        $display("lanes=%h unpacked=%h", from_lanes, from_unpacked);
        if (from_packed !== 8'hcd ||
            unpacked_from_word.hi !== 4'hc || unpacked_from_word.lo !== 4'hd ||
            packed_from_unpacked.hi !== 4'he || packed_from_unpacked.lo !== 4'hf ||
            from_desc !== 8'hab || desc_from_word[1] !== 4'hc ||
            desc_from_word[0] !== 4'hd ||
            bits_from_xz[0] !== 4'b1000 || bits_from_xz[1] !== 4'b0010) begin
            $display("FAIL bitstream_probe aggregate destinations");
            $finish;
        end
        $display("from_word=%h%h", lanes_from_word[0], lanes_from_word[1]);
        $display("PASS bitstream_probe");
        $finish;
    end
endmodule
