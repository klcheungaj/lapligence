// IEEE 1800-2009 7.2, 7.3, and 10.9.2: unpacked aggregate declaration
// patterns assign each fixed packed integral member in its own assignment
// context. Named member keys take precedence over matching type and default
// keys. Untagged unions are initialized through selected-member writes.
module tb;
    typedef struct {
        logic [63:0] upper;
        bit [63:0] lower;
    } unpacked_pair128_t;

    typedef logic [127:0] lane128_t;
    typedef struct {
        lane128_t named_override;
        lane128_t typed_match;
        bit scalar_match;
        logic [254:0] default_match;
    } unpacked_keyed512_t;

    typedef union {
        logic [127:0] logic_value;
        bit [127:0] bit_value;
    } unpacked_union128_t;

    typedef union {
        logic [511:0] logic_value;
        bit [511:0] bit_value;
    } unpacked_union512_t;

    unpacked_pair128_t positional = '{
        64'h0123_4567_89ab_cdef,
        64'hfedc_ba98_7654_3210
    };
    unpacked_pair128_t member_named = '{
        lower: 64'h0011_2233_4455_6677,
        upper: 64'h8899_aabb_ccdd_eeff
    };
    unpacked_pair128_t defaulted = '{default: '1};
    unpacked_keyed512_t type_keyed = '{
        named_override: 128'h0123456789abcdef_0011223344556677,
        lane128_t: {128{1'bz}},
        bit: 1'bx,
        default: 'x
    };

    unpacked_union128_t union_logic_selected;
    unpacked_union128_t union_bit_selected;
    unpacked_union512_t union_wide_selected;

    generate
        if (1) begin : generated_scope
            unpacked_pair128_t generated_struct = '{
                upper: 64'h1357_9bdf_2468_ace0,
                lower: 64'h0eca_8642_fdb9_7531
            };
            unpacked_union128_t generated_union;
        end
    endgenerate

    initial begin
        union_logic_selected.logic_value =
            128'h0123456789abcdef_fedcba9876543210;
        union_bit_selected.bit_value = {128{1'bx}};
        union_wide_selected.logic_value = {
            128'h0123456789abcdef_fedcba9876543210,
            128'h1111222233334444_5555666677778888,
            128'h9999aaaabbbbcccc_ddddeeeeffff0000,
            128'h89abcdef01234567_76543210fedcba98
        };
        generated_scope.generated_union.logic_value = {
            64'h0f0e_0d0c_0b0a_0908,
            64'h0706_0504_0302_0100
        };
        if (positional.upper !== 64'h0123_4567_89ab_cdef
                || positional.lower !== 64'hfedc_ba98_7654_3210
                || member_named.upper !== 64'h8899_aabb_ccdd_eeff
                || member_named.lower !== 64'h0011_2233_4455_6677
                || defaulted.upper !== {64{1'b1}}
                || defaulted.lower !== {64{1'b1}}
                || generated_scope.generated_struct.upper
                    !== 64'h1357_9bdf_2468_ace0
                || generated_scope.generated_struct.lower
                    !== 64'h0eca_8642_fdb9_7531) begin
            $display("FAIL unpacked_struct_128_patterns");
            $finish;
        end

        if (type_keyed.named_override
                    !== 128'h0123456789abcdef_0011223344556677
                || type_keyed.typed_match !== {128{1'bz}}
                || type_keyed.scalar_match !== 1'b0
                || type_keyed.default_match !== {255{1'bx}}) begin
            $display("FAIL unpacked_struct_512_type_keys");
            $finish;
        end

        if (union_logic_selected.logic_value
                    !== 128'h0123456789abcdef_fedcba9876543210
                || union_bit_selected.bit_value !== {128{1'b0}}
                || union_wide_selected.logic_value !== {
                    128'h0123456789abcdef_fedcba9876543210,
                    128'h1111222233334444_5555666677778888,
                    128'h9999aaaabbbbcccc_ddddeeeeffff0000,
                    128'h89abcdef01234567_76543210fedcba98
                }
                || generated_scope.generated_union.logic_value
                    !== 128'h0f0e0d0c0b0a0908_0706050403020100) begin
            $display("FAIL unpacked_union_member_writes");
            $finish;
        end

        $display("PASS unpacked_struct_patterns_and_union_member_writes");
        $finish;
    end
endmodule
