// SV 10.9.1-10.9.2: recursive matching, key precedence, and value contexts.
module type_key_check #(parameter int W = 7)(output bit done);
    typedef logic [W-1:0] lane_t;
    typedef bit [W-1:0] two_t;
    typedef logic signed [W-1:0] signed_t;
    typedef struct { lane_t x, y; two_t clean; } record_t;
    typedef record_t records_t[-1:1];
    typedef lane_t row_t[2:-1];
    typedef row_t matrix_t[1:0];
    typedef struct packed { lane_t upper, lower; bit flag; } packed_t;
    localparam int KEY = 0;
    localparam records_t INITIAL = '{record_t:'{x:'0, y:'0, clean:'0}};
    records_t values = INITIAL;
    records_t queued;
    record_t special;
    lane_t first_value, last_value, saved_fill;
    record_t saved_special;
    row_t explicit_row, fallback_row, semantic_indices;
    matrix_t matrix;
    lane_t unused_key[3:1];
    signed_t signed_values[2];
    signed_t negative;
    logic signed [W:0] extended;
    packed_t packed_value;

    function automatic int center_key();
        return KEY;
    endfunction
    function automatic records_t make(input record_t replacement, input lane_t fill);
        records_t local_value = '{(center_key()):replacement, lane_t:fill, two_t:two_t'(fill)};
        return local_value;
    endfunction
    function automatic records_t direct(input record_t replacement, input lane_t fill);
        return '{(KEY):replacement, lane_t:fill, two_t:two_t'(fill)};
    endfunction
    task automatic check(input records_t got, input record_t replacement, input lane_t fill);
        if (got[0].x !== replacement.x || got[0].y !== replacement.y ||
            got[0].clean !== replacement.clean)
            $fatal(1, "SYN001 W=%0d: explicit index precedence", W);
        for (int i = -1; i <= 1; i++) begin
            if (i != 0 && (got[i].x !== fill || got[i].y !== fill ||
                got[i].clean !== two_t'(fill)))
                $fatal(1, "SYN001 W=%0d: recursive type or state conversion", W);
        end
    endtask
    initial begin
        done = 0;
        if (values[-1].x !== '0 || values[0].clean !== '0 || values[1].y !== '0)
            $fatal(1, "SYN001 declaration/type-key initialization");
        first_value = '0;
        last_value = '1;
        special = '{x:'1, y:'0, clean:'1};
        values = '{(KEY+0):special, lane_t:first_value, lane_t:last_value,
                    two_t:two_t'(last_value)};
        check(values, special, last_value);
        check(make(special, last_value), special, last_value);
        check(direct(special, first_value), special, first_value);
        last_value = 'x;
        values = '{(center_key()):special, lane_t:last_value, two_t:two_t'(last_value)};
        check(values, special, last_value);
        last_value = 'z;
        check(make(special, last_value), special, last_value);

        // A matching immediate row type wins before recursive leaf matching.
        explicit_row = '{'1, '0, '1, '0};
        fallback_row = '{'0, '1, '0, '1};
        matrix = '{1:explicit_row, row_t:fallback_row, lane_t:last_value};
        for (int j = -1; j <= 2; j++) begin
            if (matrix[1][j] !== explicit_row[j] || matrix[0][j] !== fallback_row[j])
                $fatal(1, "SYN001 row-type precedence/direction");
        end
        // Index-only patterns must keep semantic keys, including constant calls.
        semantic_indices = '{(KEY+2):first_value, (KEY+1):last_value,
                              (center_key()):first_value, (-1):last_value};
        if (semantic_indices[2] !== first_value || semantic_indices[1] !== last_value ||
            semantic_indices[0] !== first_value || semantic_indices[-1] !== last_value)
            $fatal(1, "SYN001 semantic index identity");
        // No tested lane width equals int; an unused key with full coverage is legal.
        unused_key = '{int:17, default:last_value};
        if (unused_key[3] !== last_value || unused_key[1] !== last_value)
            $fatal(1, "SYN001 unused type/default");
        packed_value = '{lane_t:last_value, default:'0};
        if (packed_value.upper !== last_value || packed_value.lower !== last_value ||
            packed_value.flag !== 0) $fatal(1, "SYN001 packed structure neighbor");
        negative = '1;
        signed_values = '{signed_t:negative};
        extended = signed_values[1];
        if (extended !== {(W+1){1'b1}}) $fatal(1, "SYN001 signed leaf extension");

        last_value = '1;
        saved_fill = last_value;
        saved_special = special;
        queued <= '{(KEY):special, lane_t:last_value, two_t:two_t'(last_value)};
        last_value = '0;
        special = '{default:'0};
        #1;
        check(queued, saved_special, saved_fill);
        done = 1;
    end
endmodule
module tb;
    wire [3:0] done;
    type_key_check #(1) c1(done[0]);
    type_key_check #(7) c7(done[1]);
    type_key_check #(65) c65(done[2]);
    type_key_check #(129) c129(done[3]);
    initial begin
        wait (&done);
        $display("TYPE_KEYS_PASS");
        $finish(0);
    end
endmodule
