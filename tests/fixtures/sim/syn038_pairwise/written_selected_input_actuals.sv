// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/written_selected_input_actuals.sv
// Explicitly written selected sources flow through user-defined input actuals.
module tb;
    typedef logic [7:0] byte_t;
    typedef byte_t byte_pair_t [0:1];
    typedef byte_t byte_triple_t [0:2];
    typedef struct packed {
        byte_t selected;
        byte_t neighbor;
    } packed_pair_t;

    packed_pair_t field_source = '{selected: 8'h11, neighbor: 8'h80};
    byte_triple_t element_source = '{0: 8'h17, 1: 8'h28, 2: 8'h39};
    byte_triple_t row_source = '{0: 8'h10, 1: 8'h20, 2: 8'h73};
    byte_triple_t concat_source = '{0: 8'h11, 1: 8'h22, 2: 8'h74};
    byte_triple_t pattern_source = '{0: 8'h12, 1: 8'h23, 2: 8'h75};
    byte_t field_sibling = 8'ha1;
    byte_t element_sibling = 8'ha2;
    byte_t row_sibling = 8'ha3;
    byte_t concat_sibling = 8'ha4;
    byte_t pattern_sibling = 8'ha5;

    byte_t field_result;
    byte_t element_result;
    byte_t row_result [0:1];
    byte_t concat_result;
    byte_t pattern_result [0:2];

    task automatic capture_field(input byte_t value);
        field_result = value;
    endtask

    task automatic capture_element(input byte_t value);
        element_result = value;
    endtask

    task automatic capture_row(input byte_pair_t value);
        row_result[0] = value[0];
        row_result[1] = value[1];
    endtask

    task automatic capture_concat(input byte_t value);
        concat_result = value;
    endtask

    task automatic capture_pattern(input byte_triple_t value);
        pattern_result[0] = value[0];
        pattern_result[1] = value[1];
        pattern_result[2] = value[2];
    endtask

    initial begin
        field_source.selected = 8'h31;
        capture_field(field_source.selected);

        element_source[1] = 8'h42;
        capture_element(element_source[1]);

        row_source[0:1] = '{0: 8'h51, 1: 8'h52};
        capture_row(row_source[0:1]);

        {concat_source[1], concat_source[0]} = {8'hA6, 8'hC2};
        capture_concat(concat_source[1]);

        byte_pair_t'{pattern_source[0], pattern_source[1]} = '{0: 8'h5c, 1: 8'h7d};
        capture_pattern(pattern_source);

        if (field_result !== 8'h31 || field_source.neighbor !== 8'h80)
            $fatal(1, "field input or neighbor mismatch");
        if (field_sibling !== 8'ha1)
            $fatal(1, "field sibling source changed");
        if (element_result !== 8'h42 || element_source[0] !== 8'h17)
            $fatal(1, "element input or neighbor mismatch");
        if (element_sibling !== 8'ha2)
            $fatal(1, "element sibling source changed");
        if (row_result[0] !== 8'h51 || row_result[1] !== 8'h52 || row_source[2] !== 8'h73)
            $fatal(1, "row-slice input or neighbor mismatch");
        if (row_sibling !== 8'ha3)
            $fatal(1, "row-slice sibling source changed");
        if (concat_result !== 8'hA6 || concat_source[2] !== 8'h74)
            $fatal(1, "concatenation input or neighbor mismatch");
        if (concat_sibling !== 8'ha4)
            $fatal(1, "concatenation sibling source changed");
        if (pattern_result[0] !== 8'h5c || pattern_result[1] !== 8'h7d ||
            pattern_result[2] !== 8'h75 || pattern_source[2] !== 8'h75)
            $fatal(1, "positional-pattern input or neighbor mismatch");
        if (pattern_sibling !== 8'ha5)
            $fatal(1, "positional-pattern sibling source changed");

        $display("selected-inputs=31/80,42/17,51,52/73,a6/74,5c,7d/75");
        $finish(0);
    end
endmodule
