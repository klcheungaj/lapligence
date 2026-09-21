// llg-test-fixture: tests/fixtures/sim/syn012_fixed_layout/matrix.sv
// IEEE 1800-2009 §§6.22, 7.2-7.4, 10.8, and 11.2.2: fixed integral
// aggregate layout and value-context matrix for SYN-012.
`ifndef SYN012_W
`define SYN012_W 8
`endif
module tb;
    localparam int W = `SYN012_W;
    typedef logic signed [W-1:0] logic_word_t;
    typedef bit [W-1:0] bit_word_t;
    typedef struct packed {
        logic signed [W-1:0] logic_part;
        bit [W-1:0] bit_part;
    } packed_record_t;
    typedef union packed {
        logic [2*W-1:0] word;
        packed_record_t record;
    } packed_union_t;
    typedef struct {
        logic signed [W-1:0] logic_part;
        bit [W-1:0] bit_part;
    } unpacked_record_t;
    typedef union {
        logic [2*W-1:0] word;
        unpacked_record_t record;
    } unpacked_union_t;
    typedef unpacked_record_t record_row_t [1:0];
    typedef record_row_t record_grid_t [2:1];

    packed_record_t packed_decl;
    packed_record_t packed_same;
    packed_union_t packed_overlay;
    unpacked_record_t unpacked_decl;
    unpacked_record_t unpacked_same;
    unpacked_union_t unpacked_overlay;
    record_grid_t grid_decl;
    record_row_t row_same;

    function automatic packed_record_t packed_return(input packed_record_t value);
        packed_record_t local_value;
        local_value = value;
        local_value.logic_part = local_value.logic_part ^ logic_word_t'(1);
        return local_value;
    endfunction

    function automatic packed_union_t packed_union_return(input packed_union_t value);
        packed_union_t local_value;
        local_value = value;
        local_value.record.logic_part =
            local_value.record.logic_part ^ logic_word_t'(1);
        return local_value;
    endfunction

    function automatic unpacked_record_t unpacked_return(input unpacked_record_t value);
        unpacked_record_t local_value;
        local_value = value;
        local_value.logic_part = local_value.logic_part ^ logic_word_t'(1);
        return local_value;
    endfunction

    function automatic record_grid_t grid_return(input record_grid_t value);
        record_grid_t local_value;
        local_value = value;
        local_value[2][1].logic_part =
            local_value[2][1].logic_part ^ logic_word_t'(1);
        return local_value;
    endfunction

    task automatic write_record(output unpacked_record_t value);
        value.logic_part = logic_word_t'(3);
        value.bit_part = bit_word_t'(5);
    endtask

    initial begin
        packed_decl = '{logic_part: logic_word_t'(8), bit_part: bit_word_t'(3)};
        packed_overlay.word = {logic_word_t'(8), bit_word_t'(3)};
        if (packed_overlay.record.logic_part !== logic_word_t'(8) ||
            packed_overlay.record.bit_part !== bit_word_t'(3)) $fatal(1, "packed union view");
        if (packed_decl != packed_overlay.record) $fatal(1, "packed equality");
        packed_decl = packed_return(packed_decl);
        if (packed_decl.logic_part !== logic_word_t'(9)) $fatal(1, "packed return");
        packed_overlay = packed_union_return(packed_overlay);
        if (packed_overlay.record.logic_part !== logic_word_t'(9))
            $fatal(1, "packed union local");
        packed_decl = (1'b1 ? packed_decl : packed_overlay.record);
        if (packed_decl.logic_part !== logic_word_t'(9)) $fatal(1, "packed conditional");
        packed_same = packed_decl;
        packed_decl = (1'bx ? packed_decl : packed_same);
        if (packed_decl !== packed_same) $fatal(1, "packed ambiguous conditional");

        write_record(unpacked_decl);
        if (unpacked_decl.logic_part !== logic_word_t'(3) ||
            unpacked_decl.bit_part !== bit_word_t'(5))
            $fatal(1, "unpacked output");
        unpacked_overlay.record = unpacked_decl;
        if (unpacked_overlay.word !== {logic_word_t'(3), bit_word_t'(5)})
            $fatal(1, "unpacked union view");
        unpacked_decl = unpacked_return(unpacked_overlay.record);
        if (unpacked_decl.logic_part !== logic_word_t'(2)) $fatal(1, "unpacked return");
        unpacked_decl = (1'b1 ? unpacked_decl : unpacked_overlay.record);
        if (unpacked_decl.logic_part !== logic_word_t'(2)) $fatal(1, "unpacked conditional");
        unpacked_same = unpacked_decl;
        unpacked_decl = (1'bx ? unpacked_decl : unpacked_same);
        if (unpacked_decl !== unpacked_same) $fatal(1, "unpacked ambiguous conditional");

        grid_decl[2][1] = unpacked_decl;
        grid_decl[2][0] = unpacked_overlay.record;
        grid_decl[1][1] = '{logic_part: logic_word_t'(7), bit_part: bit_word_t'(1)};
        grid_decl[1][0] = '{logic_part: logic_word_t'(6), bit_part: bit_word_t'(2)};
        if (grid_decl[2][1].logic_part !== logic_word_t'(2) ||
            grid_decl[1][0].bit_part !== bit_word_t'(2)) $fatal(1, "array row member");
        grid_decl = grid_return(grid_decl);
        if (grid_decl[2][1].logic_part !== logic_word_t'(3)) $fatal(1, "array return");
        if (grid_decl[2] !== grid_decl[2]) $fatal(1, "array equality");
        row_same = grid_decl[2];
        grid_decl[2] = (1'bx ? grid_decl[2] : row_same);
        if (grid_decl[2] !== row_same) $fatal(1, "array conditional");
        $display("PASS syn012_fixed_layout W=%0d", W);
        $finish(0);
    end
endmodule
