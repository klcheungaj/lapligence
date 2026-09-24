// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/written_ref_inout_actuals.sv
// Blocking writes through selected lvalues feed whole-variable ref and inout actuals.
module tb;
    typedef logic [7:0] byte_t;
    typedef byte_t byte_pair_t [0:1];
    typedef byte_t byte_triple_t [0:2];
    typedef struct packed {
        byte_t selected;
        byte_t neighbor;
    } packed_pair_t;

    byte_t whole_ref_source = 8'h00;
    packed_pair_t field_ref_source = '{selected: 8'h00, neighbor: 8'h80};
    byte_triple_t element_ref_source = '{0: 8'h11, 1: 8'h22, 2: 8'h33};
    byte_triple_t row_ref_source = '{0: 8'h10, 1: 8'h20, 2: 8'h76};
    byte_triple_t concat_ref_source = '{0: 8'h11, 1: 8'h22, 2: 8'h74};
    byte_triple_t pattern_ref_source = '{0: 8'h12, 1: 8'h23, 2: 8'h75};

    byte_t whole_inout_source = 8'h00;
    packed_pair_t field_inout_source = '{selected: 8'h00, neighbor: 8'h81};
    byte_triple_t element_inout_source = '{0: 8'h12, 1: 8'h24, 2: 8'h34};
    byte_triple_t row_inout_source = '{0: 8'h30, 1: 8'h40, 2: 8'h77};
    byte_triple_t concat_inout_source = '{0: 8'h31, 1: 8'h42, 2: 8'h78};
    byte_triple_t pattern_inout_source = '{0: 8'h32, 1: 8'h43, 2: 8'h79};

    byte_t seen_whole_ref;
    byte_t seen_field_ref;
    byte_t seen_element_ref;
    byte_t seen_row_ref;
    byte_t seen_concat_ref;
    byte_t seen_pattern_ref;
    byte_t seen_whole_inout;
    byte_t seen_field_inout;
    byte_t seen_element_inout;
    byte_t seen_row_inout;
    byte_t seen_concat_inout;
    byte_t seen_pattern_inout;

    task automatic mutate_scalar_ref(ref byte_t value);
        seen_whole_ref = value;
        value = value ^ 8'hff;
    endtask

    task automatic mutate_scalar_inout(inout byte_t value);
        seen_whole_inout = value;
        value = value ^ 8'hff;
    endtask

    task automatic mutate_field_ref(ref packed_pair_t value);
        seen_field_ref = value.selected;
        value.selected = value.selected ^ 8'hff;
    endtask

    task automatic mutate_field_inout(inout packed_pair_t value);
        seen_field_inout = value.selected;
        value.selected = value.selected ^ 8'hff;
    endtask

    task automatic mutate_array_ref(
        ref byte_triple_t value,
        input int index,
        output byte_t observed
    );
        observed = value[index];
        value[index] = value[index] ^ 8'hff;
    endtask

    task automatic mutate_array_inout(
        inout byte_triple_t value,
        input int index,
        output byte_t observed
    );
        observed = value[index];
        value[index] = value[index] ^ 8'hff;
    endtask

    initial begin
        whole_ref_source = 8'h10;
        mutate_scalar_ref(whole_ref_source);

        field_ref_source.selected = 8'h21;
        mutate_field_ref(field_ref_source);

        element_ref_source[1] = 8'h32;
        mutate_array_ref(element_ref_source, 1, seen_element_ref);

        row_ref_source[0:1] = '{0: 8'h43, 1: 8'h54};
        mutate_array_ref(row_ref_source, 1, seen_row_ref);

        {concat_ref_source[1], concat_ref_source[0]} = {8'hA6, 8'hC2};
        mutate_array_ref(concat_ref_source, 1, seen_concat_ref);

        byte_pair_t'{pattern_ref_source[0], pattern_ref_source[1]} = '{0: 8'h5c, 1: 8'h7d};
        mutate_array_ref(pattern_ref_source, 0, seen_pattern_ref);

        whole_inout_source = 8'h11;
        mutate_scalar_inout(whole_inout_source);

        field_inout_source.selected = 8'h22;
        mutate_field_inout(field_inout_source);

        element_inout_source[1] = 8'h42;
        mutate_array_inout(element_inout_source, 1, seen_element_inout);

        row_inout_source[0:1] = '{0: 8'h53, 1: 8'h64};
        mutate_array_inout(row_inout_source, 1, seen_row_inout);

        {concat_inout_source[1], concat_inout_source[0]} = {8'hB6, 8'hD2};
        mutate_array_inout(concat_inout_source, 1, seen_concat_inout);

        byte_pair_t'{pattern_inout_source[0], pattern_inout_source[1]} = '{0: 8'h6c, 1: 8'h8d};
        mutate_array_inout(pattern_inout_source, 0, seen_pattern_inout);

        if (seen_whole_ref !== 8'h10 || whole_ref_source !== 8'hef)
            $fatal(1, "whole ref did not read the blocking write before modifying it");
        if (seen_field_ref !== 8'h21 || field_ref_source.selected !== 8'hde ||
            field_ref_source.neighbor !== 8'h80)
            $fatal(1, "field ref did not preserve the selected write and neighbor");
        if (seen_element_ref !== 8'h32 || element_ref_source[1] !== 8'hcd ||
            element_ref_source[0] !== 8'h11 || element_ref_source[2] !== 8'h33)
            $fatal(1, "element ref did not preserve the selected write and neighbors");
        if (seen_row_ref !== 8'h54 || row_ref_source[0] !== 8'h43 ||
            row_ref_source[1] !== 8'hab || row_ref_source[2] !== 8'h76)
            $fatal(1, "row-slice ref did not preserve the selected write and neighbor");
        if (seen_concat_ref !== 8'ha6 || concat_ref_source[0] !== 8'hc2 ||
            concat_ref_source[1] !== 8'h59 || concat_ref_source[2] !== 8'h74)
            $fatal(1, "concatenation ref did not preserve the selected write and neighbor");
        if (seen_pattern_ref !== 8'h5c || pattern_ref_source[0] !== 8'ha3 ||
            pattern_ref_source[1] !== 8'h7d || pattern_ref_source[2] !== 8'h75)
            $fatal(1, "positional-pattern ref did not preserve the selected write and neighbors");

        if (seen_whole_inout !== 8'h11 || whole_inout_source !== 8'hee)
            $fatal(1, "whole inout did not read the blocking write before modifying it");
        if (seen_field_inout !== 8'h22 || field_inout_source.selected !== 8'hdd ||
            field_inout_source.neighbor !== 8'h81)
            $fatal(1, "field inout did not preserve the selected write and neighbor");
        if (seen_element_inout !== 8'h42 || element_inout_source[1] !== 8'hbd ||
            element_inout_source[0] !== 8'h12 || element_inout_source[2] !== 8'h34)
            $fatal(1, "element inout did not preserve the selected write and neighbors");
        if (seen_row_inout !== 8'h64 || row_inout_source[0] !== 8'h53 ||
            row_inout_source[1] !== 8'h9b || row_inout_source[2] !== 8'h77)
            $fatal(1, "row-slice inout did not preserve the selected write and neighbor");
        if (seen_concat_inout !== 8'hb6 || concat_inout_source[0] !== 8'hd2 ||
            concat_inout_source[1] !== 8'h49 || concat_inout_source[2] !== 8'h78)
            $fatal(1, "concatenation inout did not preserve the selected write and neighbor");
        if (seen_pattern_inout !== 8'h6c || pattern_inout_source[0] !== 8'h93 ||
            pattern_inout_source[1] !== 8'h8d || pattern_inout_source[2] !== 8'h79)
            $fatal(1, "positional-pattern inout did not preserve the selected write and neighbors");

        $display("prior-ref=10>ef,21>de,32>cd,54>ab,a6>59,5c>a3");
        $display("prior-inout=11>ee,22>dd,42>bd,64>9b,b6>49,6c>93");
        $finish(0);
    end
endmodule
