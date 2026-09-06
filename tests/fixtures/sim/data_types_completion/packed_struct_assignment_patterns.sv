// IEEE 1800-2009 6.22.1, 7.2.1, and 10.9.2: packed-structure assignment
// patterns map positional values in declaration order. Named members override
// matching type keys, and matching type keys override default values. Type
// keys use matching types, not merely equal flattened widths.
module tb;
    typedef struct packed {
        logic [63:0] upper;
        bit [63:0] lower;
    } pair128_t;

    typedef logic [127:0] lane128_t;
    typedef struct packed {
        lane128_t named_override;
        lane128_t typed_match;
        bit scalar_match;
        logic [254:0] default_match;
    } keyed512_t;

    typedef logic [7:0] canonical_lane_t;
    typedef canonical_lane_t matching_alias_t;
    typedef logic [0:7] reverse_lane_t;
    typedef logic [1:0][3:0] multidimensional_lane_t;
    typedef logic signed [7:0] signed_lane_t;
    typedef bit [7:0] two_state_lane_t;
    typedef struct packed {
        canonical_lane_t direct_match;
        matching_alias_t alias_match;
        reverse_lane_t reverse_no_match;
        multidimensional_lane_t dimensions_no_match;
        signed_lane_t signed_no_match;
        two_state_lane_t state_no_match;
    } matching_controls_t;

    typedef logic [15:0] priority_lane_t;
    typedef struct packed {
        priority_lane_t lane;
    } priority_controls_t;

    pair128_t positional = '{
        64'h0123_4567_89ab_cdef,
        64'hfedc_ba98_7654_3210
    };
    pair128_t member_named = '{
        lower: 64'h0011_2233_4455_6677,
        upper: 64'h8899_aabb_ccdd_eeff
    };
    pair128_t defaulted = '{default: '1};
    keyed512_t type_keyed = '{
        named_override: 128'h0123456789abcdef_0011223344556677,
        lane128_t: {128{1'bz}},
        bit: 1'bx,
        default: 'x
    };
    matching_controls_t matching_controls = '{
        canonical_lane_t: 8'ha5,
        default: 8'h3c
    };
    priority_controls_t named_then_type = '{
        lane: 16'h1357,
        priority_lane_t: 16'h2468
    };
    priority_controls_t type_then_named = '{
        priority_lane_t: 16'h2468,
        lane: 16'h1357
    };

    generate
        if (1) begin : generated_scope
            pair128_t generated = '{
                upper: 64'h1357_9bdf_2468_ace0,
                lower: 64'h0eca_8642_fdb9_7531
            };
        end
    endgenerate

    initial begin
        if ($bits(positional) != 128
                || positional.upper !== 64'h0123_4567_89ab_cdef
                || positional.lower !== 64'hfedc_ba98_7654_3210
                || member_named.upper !== 64'h8899_aabb_ccdd_eeff
                || member_named.lower !== 64'h0011_2233_4455_6677
                || defaulted.upper !== {64{1'b1}}
                || defaulted.lower !== {64{1'b1}}
                || generated_scope.generated.upper !== 64'h1357_9bdf_2468_ace0
                || generated_scope.generated.lower !== 64'h0eca_8642_fdb9_7531) begin
            $display("FAIL packed_struct_128_patterns");
            $finish;
        end

        if ($bits(type_keyed) != 512
                || type_keyed.named_override
                    !== 128'h0123456789abcdef_0011223344556677
                || type_keyed.typed_match !== {128{1'bz}}
                || type_keyed.scalar_match !== 1'b0
                || type_keyed.default_match !== {255{1'bx}}) begin
            $display("FAIL packed_struct_512_type_keys");
            $finish;
        end

        if (matching_controls.direct_match !== 8'ha5
                || matching_controls.alias_match !== 8'ha5
                || matching_controls.reverse_no_match !== 8'h3c
                || matching_controls.dimensions_no_match !== 8'h3c
                || matching_controls.signed_no_match !== 8'h3c
                || matching_controls.state_no_match !== 8'h3c) begin
            $display("FAIL packed_struct_type_key_matching_controls");
            $finish;
        end

        if (named_then_type.lane !== 16'h1357
                || type_then_named.lane !== 16'h1357) begin
            $display("FAIL packed_struct_member_over_type_precedence");
            $finish;
        end

        $display("PASS packed_struct_assignment_patterns");
        $finish;
    end
endmodule
