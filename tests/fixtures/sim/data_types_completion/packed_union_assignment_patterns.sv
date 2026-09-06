// IEEE 1800-2009 7.3.1 and 10.9: a packed untagged union has one shared
// integral representation. A declaration assignment pattern selects exactly
// one named member; writes through a two-state member convert X/Z to zero.
module tb;
    typedef struct packed {
        logic [63:0] upper;
        logic [63:0] lower;
    } halves128_t;

    typedef union packed {
        logic [127:0] logic_view;
        bit [127:0] bit_view;
        halves128_t halves;
    } union128_t;

    typedef union packed {
        logic [511:0] logic_view;
        bit [511:0] bit_view;
    } union512_t;

    union128_t logic_selected = '{
        logic_view: 128'h0123456789abcdef_fedcba9876543210
    };
    union128_t bit_selected = '{bit_view: {128{1'bx}}};
    union128_t struct_selected = '{halves: '{
        upper: 64'haaaa_5555_ffff_0000,
        lower: 64'h1357_9bdf_2468_ace0
    }};
    union512_t wide_selected = '{logic_view: {
        128'h0123456789abcdef_fedcba9876543210,
        128'h1111222233334444_5555666677778888,
        128'h9999aaaabbbbcccc_ddddeeeeffff0000,
        128'h89abcdef01234567_76543210fedcba98
    }};

    generate
        if (1) begin : generated_scope
            union128_t generated = '{logic_view: {
                64'h0f0e_0d0c_0b0a_0908,
                64'h0706_0504_0302_0100
            }};
        end
    endgenerate

    initial begin
        if ($bits(logic_selected) != 128
                || logic_selected.logic_view
                    !== 128'h0123456789abcdef_fedcba9876543210
                || logic_selected.halves.upper !== 64'h0123_4567_89ab_cdef
                || logic_selected.halves.lower !== 64'hfedc_ba98_7654_3210
                || bit_selected.logic_view !== {128{1'b0}}
                || struct_selected.logic_view
                    !== 128'haaaa5555ffff0000_13579bdf2468ace0
                || generated_scope.generated.logic_view
                    !== 128'h0f0e0d0c0b0a0908_0706050403020100) begin
            $display("FAIL packed_union_128_patterns");
            $finish;
        end

        if ($bits(wide_selected) != 512
                || wide_selected.logic_view !== {
                    128'h0123456789abcdef_fedcba9876543210,
                    128'h1111222233334444_5555666677778888,
                    128'h9999aaaabbbbcccc_ddddeeeeffff0000,
                    128'h89abcdef01234567_76543210fedcba98
                }) begin
            $display("FAIL packed_union_512_pattern");
            $finish;
        end

        $display("PASS packed_union_assignment_patterns");
        $finish;
    end
endmodule
