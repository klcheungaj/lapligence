// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/constref_written_source_actuals.sv
// Prior whole-variable writes remain observable through const-ref task actuals.
module tb;
    typedef logic [7:0] byte_t;
    typedef byte_t byte_pair_t [0:1];
    typedef byte_t byte_triple_t [0:2];
    typedef struct packed {
        byte_t selected;
        byte_t neighbor;
    } packed_pair_t;

    byte_t whole_source = 8'h10;
    packed_pair_t field_source = '{selected: 8'h11, neighbor: 8'h81};
    byte_triple_t element_source = '{0: 8'h16, 1: 8'h27, 2: 8'h75};
    byte_triple_t row_source = '{0: 8'h18, 1: 8'h29, 2: 8'h76};
    byte_triple_t concat_source = '{0: 8'h1a, 1: 8'h2b, 2: 8'h77};
    byte_triple_t pattern_source = '{0: 8'h1c, 1: 8'h2d, 2: 8'h78};
    logic [7:0] nba_source = 8'h30;
    logic [7:0] continuous_source;
    byte_t sibling_control = 8'hd0;

    byte_t whole_result;
    byte_t field_result;
    byte_t field_neighbor_result;
    byte_triple_t element_result;
    byte_triple_t row_result;
    byte_triple_t concat_result;
    byte_triple_t pattern_result;
    byte_t nba_result;
    byte_t continuous_result;

    assign continuous_source = 8'h97;

    task automatic capture_whole(const ref byte_t value);
        whole_result = value;
    endtask

    task automatic capture_field(const ref packed_pair_t value);
        field_result = value.selected;
        field_neighbor_result = value.neighbor;
    endtask

    task automatic capture_element(const ref byte_triple_t value);
        element_result = value;
    endtask

    task automatic capture_row(const ref byte_triple_t value);
        row_result = value;
    endtask

    task automatic capture_concat(const ref byte_triple_t value);
        concat_result = value;
    endtask

    task automatic capture_pattern(const ref byte_triple_t value);
        pattern_result = value;
    endtask

    task automatic capture_nba(const ref byte_t value);
        nba_result = value;
    endtask

    task automatic capture_continuous(const ref byte_t value);
        continuous_result = value;
    endtask

    initial begin
        whole_source = 8'h31;
        capture_whole(whole_source);

        field_source.selected = 8'h42;
        capture_field(field_source);

        element_source[1] = 8'h53;
        capture_element(element_source);

        row_source[0:1] = '{0: 8'h51, 1: 8'h62};
        capture_row(row_source);

        {concat_source[1], concat_source[0]} = {8'hA7, 8'hB8};
        capture_concat(concat_source);

        byte_pair_t'{pattern_source[0], pattern_source[1]} = '{0: 8'h8d, 1: 8'h9e};
        capture_pattern(pattern_source);

        nba_source <= 8'h86;
        #1;
        capture_nba(nba_source);
        capture_continuous(continuous_source);

        if (whole_source !== 8'h31 || whole_result !== 8'h31)
            $fatal(1, "whole const-ref source mismatch");
        if (field_source.selected !== 8'h42 || field_source.neighbor !== 8'h81 ||
            field_result !== 8'h42 || field_neighbor_result !== 8'h81)
            $fatal(1, "field const-ref source mismatch");
        if (element_source[0] !== 8'h16 || element_source[1] !== 8'h53 ||
            element_source[2] !== 8'h75 || element_result !== element_source)
            $fatal(1, "element const-ref source mismatch");
        if (row_source[0] !== 8'h51 || row_source[1] !== 8'h62 ||
            row_source[2] !== 8'h76 || row_result !== row_source)
            $fatal(1, "row-slice const-ref source mismatch");
        if (concat_source[0] !== 8'hB8 || concat_source[1] !== 8'hA7 ||
            concat_source[2] !== 8'h77 || concat_result !== concat_source)
            $fatal(1, "concatenation const-ref source mismatch");
        if (pattern_source[0] !== 8'h8d || pattern_source[1] !== 8'h9e ||
            pattern_source[2] !== 8'h78 || pattern_result !== pattern_source)
            $fatal(1, "positional-pattern const-ref source mismatch");
        if (nba_source !== 8'h86 || nba_result !== 8'h86)
            $fatal(1, "NBA const-ref source mismatch");
        if (continuous_source !== 8'h97 || continuous_result !== 8'h97)
            $fatal(1, "continuous-variable const-ref source mismatch");
        if (sibling_control !== 8'hd0)
            $fatal(1, "independent sibling source changed");

        $display("constref=31,42/81,53/16/75,51,62/76,a7,b8/77,8d,9e/78");
        $display("extra=86/97");
        $finish(0);
    end
endmodule
