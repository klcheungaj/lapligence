// llg-test-fixture: SYN-012 nested fixed values, nominal types, state and signedness.
`ifndef CONTINUATION_LAYOUT_W
`define CONTINUATION_LAYOUT_W 65
`endif
module tb;
    localparam int W = `CONTINUATION_LAYOUT_W;
    typedef logic signed [W-1:0] signed_t;
    typedef bit [W-1:0] known_t;
    typedef union packed { logic [W-1:0] raw; signed_t signed_view; } overlay_t;
    typedef struct packed { logic marker; signed_t data; known_t flags; } packed_t;
    typedef signed_t row_t[1:-1];
    typedef struct { row_t lanes; known_t flags; overlay_t overlay; } record_t;
    typedef record_t cube_t[-1:0][2:1];
    typedef union { signed_t first; logic [W-1:0] second; } unpacked_view_t;
    cube_t source, copy, changed;
    record_t selected;
    packed_t packed_value;
    unpacked_view_t unpacked_value, unpacked_copy;
    int outer_index, inner_index;
    logic unknown_select;
    logic [W+1:0] widened;

    function automatic cube_t modified(input cube_t value, input int outer, inner);
        cube_t local_copy;
        local_copy = value;
        local_copy[outer][inner].lanes[0] ^= signed_t'(1);
        return local_copy;
    endfunction
    function automatic record_t select_record(input cube_t value, input int outer, inner);
        record_t local_record;
        local_record = value[outer][inner];
        return local_record;
    endfunction
    function automatic unpacked_view_t copy_union(input unpacked_view_t value);
        return value;
    endfunction

    initial begin
        for (int i = -1; i <= 0; i++) begin
            for (int j = 2; j >= 1; j--) begin
                source[i][j].lanes = '{signed_t'(i + j), signed_t'(-1), '0};
                source[i][j].flags = '1;
                source[i][j].overlay.raw = '1;
            end
        end
        copy = source;
        if (copy !== source) $fatal(1, "deep copy");
        outer_index = 0;
        inner_index = 1;
        changed = modified(source, outer_index, inner_index);
        if (source[0][1].lanes[0] !== signed_t'(-1) ||
            changed[0][1].lanes[0] !== (signed_t'(-1) ^ signed_t'(1)))
            $fatal(1, "automatic copy isolation");
        if (changed[-1][2] !== source[-1][2]) $fatal(1, "neighbor row");
        selected = select_record(source, outer_index, inner_index);
        widened = selected.overlay.signed_view;
        if (widened !== {(W+2){1'b1}}) $fatal(1, "signed overlay projection");
        selected.flags = 'x;
        if (selected.flags !== '0 || source[0][1].flags !== '1)
            $fatal(1, "member conversion isolation");

        packed_value = {1'bx, {W{1'b1}}, {W{1'bx}}};
        if (packed_value.marker !== 1'bx || packed_value.data !== signed_t'(-1) ||
            packed_value.flags !== '0) $fatal(1, "packed member domains");
        widened = packed_value.data;
        if (widened !== {(W+2){1'b1}}) $fatal(1, "packed member sign");
        unpacked_value.first = signed_t'(-1);
        unpacked_copy = copy_union(unpacked_value);
        if (unpacked_copy.first !== signed_t'(-1)) $fatal(1, "unpacked union value");

        source[0][1].lanes[-1] = 'x;
        copy = source;
        if ((source == copy) !== 1'bx || (source === copy) !== 1'b1)
            $fatal(1, "recursive equality state");
        copy[-1][2].flags = '0;
        if ((source == copy) !== 1'b0) $fatal(1, "known mismatch dominates");
        unknown_select = 1'bx;
        selected = unknown_select ? source[-1][2] : copy[-1][2];
        if (selected.lanes !== source[-1][2].lanes || selected.flags !== '0 ||
            selected.overlay.raw !== source[-1][2].overlay.raw)
            $fatal(1, "record immediate-member default");
        changed <= source;
        source[0][1].lanes[-1] = '0;
        #1;
        if (changed[0][1].lanes[-1] !== {W{1'bx}}) $fatal(1, "deep NBA snapshot");
        $display("LAYOUT_CONTEXTS_PASS W=%0d", W);
        $finish(0);
    end
endmodule
