// SV 10.9.1: recursive type/default keys, last-type precedence and bound order.
module tb;
    typedef int row_t[-2:-3];
    typedef logic [7:0] lane_t;
    row_t rows[1:0];
    row_t explicit_row;
    lane_t lanes[3:1];
    int first_value, last_value;
    lane_t replacement, fallback;
    initial begin
        first_value = 7;
        last_value = 9;
        explicit_row = '{11, 22};
        rows = '{1:explicit_row, int:first_value, int:last_value};
        if (rows[1][-2] != 11 || rows[1][-3] != 22 ||
            rows[0][-2] != 9 || rows[0][-3] != 9)
            $fatal(1, "mixed descending rows/last type");
        rows = '{int:first_value};
        if (rows[1][-2] != 7 || rows[0][-3] != 7)
            $fatal(1, "type-only recursive control");
        replacement = 8'h12;
        fallback = 8'h34;
        lanes = '{(1+1):replacement, int:first_value, default:fallback};
        if (lanes[3] !== 8'h34 || lanes[2] !== 8'h12 || lanes[1] !== 8'h34)
            $fatal(1, "unused key/default/explicit precedence");
        $display("PASS n03_mixed_row_patterns");
        $finish(0);
    end
endmodule
