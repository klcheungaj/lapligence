// llg-test-fixture: IEEE 1800-2009 §§7.6, 23.2.2, and 23.3.3. Fixed-array
// input ports accept value expressions and runtime-selected source rows.
typedef logic [7:0] lane_t;
typedef lane_t row_t [0:1];
typedef lane_t reverse_row_t [1:0];
typedef lane_t triple_t [0:2];

module child #(parameter lane_t BIAS = 0) (
    input row_t a,
    output lane_t y [1:0]
);
    // The formal uses the opposite unpacked direction. Values still map by
    // declaration-order position: source first element reaches y[1].
    assign y[1] = a[0] + BIAS;
    assign y[0] = a[1] + BIAS;
endmodule

module tb;
    row_t left;
    row_t right;
    reverse_row_t reverse;
    row_t matrix [0:1];
    row_t conditional_y;
    row_t function_y;
    row_t selected_y;
    triple_t slice_source;
    row_t slice_y;
    row_t pattern_y;
    row_t reverse_y;
    logic select;
    integer index;
    integer calls;

    function automatic row_t make_row(input row_t value);
        begin
            calls = calls + 1;
            make_row = value;
        end
    endfunction

    child #(.BIAS(1)) conditional(.a(select ? left : right), .y(conditional_y));
    child #(.BIAS(2)) function_value(.a(make_row(left)), .y(function_y));
    child #(.BIAS(3)) selected_row(.a(matrix[index]), .y(selected_y));
    child #(.BIAS(4)) fixed_slice(.a(slice_source[0:1]), .y(slice_y));
    child #(.BIAS(5)) typed_pattern(.a('{8'h12, 8'h34}), .y(pattern_y));
    child #(.BIAS(6)) opposite_input(.a(reverse), .y(reverse_y));

    initial begin
        calls = 0;
        left[0] = 8'h10;
        left[1] = 8'h20;
        right[0] = 8'ha0;
        right[1] = 8'hb0;
        reverse[1] = 8'h55;
        reverse[0] = 8'h66;
        matrix[0][0] = 8'h30;
        matrix[0][1] = 8'h40;
        matrix[1][0] = 8'hc0;
        matrix[1][1] = 8'hd0;
        slice_source[0] = 8'h10;
        slice_source[1] = 8'h20;
        slice_source[2] = 8'h30;
        select = 1'b0;
        index = 0;
        #1;
        $display("t1 cond=%h,%h func=%h,%h selected=%h,%h slice=%h,%h pattern=%h,%h reverse=%h,%h calls=%0d",
                 conditional_y[0], conditional_y[1], function_y[0], function_y[1],
                 selected_y[0], selected_y[1], slice_y[0], slice_y[1],
                 pattern_y[0], pattern_y[1], reverse_y[0], reverse_y[1], calls);

        select = 1'b1;
        index = 1;
        left[1] = 8'h2f;
        matrix[1][0] = 8'he0;
        #1;
        $display("t2 cond=%h,%h func=%h,%h selected=%h,%h slice=%h,%h pattern=%h,%h reverse=%h,%h calls=%0d",
                 conditional_y[0], conditional_y[1], function_y[0], function_y[1],
                 selected_y[0], selected_y[1], slice_y[0], slice_y[1],
                 pattern_y[0], pattern_y[1], reverse_y[0], reverse_y[1], calls);
        $finish(0);
    end
endmodule
