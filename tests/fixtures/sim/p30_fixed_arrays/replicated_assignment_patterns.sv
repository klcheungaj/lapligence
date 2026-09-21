// IEEE 1800-2009 10.9.1 and 10.9.2: replicated assignment patterns retain
// their element order and may contain nested fixed-array and structure keys.
module tb;
    typedef logic [7:0] lane_t;
    typedef lane_t lane_array_t [0:1];
    typedef lane_t row_t [3:2];
    typedef struct { lane_t a; lane_t b; } pair_t;

    lane_t y;
    lane_array_t values;
    lane_array_t returned;
    row_t reversed;
    lane_t matrix [1:0][3:2];
    row_t nested_defaults [0:1];
    pair_t typed_pairs [0:1];
    logic [1:0][7:0] packed_rows;

    function automatic lane_array_t make_array(input lane_t value);
        make_array = '{2{value}};
    endfunction

    task automatic fill(output lane_array_t target, input lane_t value);
        target = '{2{value}};
    endtask

    initial begin
        y = 8'h5a;
        values = '{2{y}};
        if (values[0] !== 8'h5a || values[1] !== 8'h5a) begin
            $display("FAIL replicated_values");
            $finish;
        end

        matrix = '{2{'{2{y}}}};
        if (matrix[1][3] !== 8'h5a || matrix[1][2] !== 8'h5a ||
            matrix[0][3] !== 8'h5a || matrix[0][2] !== 8'h5a) begin
            $display("FAIL replicated_nested_rows");
            $finish;
        end

        nested_defaults = '{2{'{default: y}}};
        if (nested_defaults[0][3] !== 8'h5a || nested_defaults[0][2] !== 8'h5a ||
            nested_defaults[1][3] !== 8'h5a || nested_defaults[1][2] !== 8'h5a) begin
            $display("FAIL replicated_nested_default");
            $finish;
        end

        typed_pairs = '{2{'{lane_t: 8'h6b}}};
        if (typed_pairs[0].a !== 8'h6b || typed_pairs[0].b !== 8'h6b ||
            typed_pairs[1].a !== 8'h6b || typed_pairs[1].b !== 8'h6b) begin
            $display("FAIL replicated_type_key");
            $finish;
        end

        reversed = '{2{8'ha6}};
        if (reversed[3] !== 8'ha6 || reversed[2] !== 8'ha6) begin
            $display("FAIL replicated_reversed_range");
            $finish;
        end

        packed_rows = '{2{y}};
        if (packed_rows !== 16'h5a5a) begin
            $display("FAIL replicated_packed_rows");
            $finish;
        end

        values = lane_array_t'('{2{8'hc3}});
        returned = make_array(y);
        if (values[0] !== 8'hc3 || values[1] !== 8'hc3 ||
            returned[0] !== 8'h5a || returned[1] !== 8'h5a) begin
            $display("FAIL replicated_typedef_or_return");
            $finish;
        end

        fill(values, 8'h3c);
        values <= '{2{8'h7e}};
        #1;
        if (values[0] !== 8'h7e || values[1] !== 8'h7e) begin
            $display("FAIL replicated_call_or_nba");
            $finish;
        end
        $display("PASS replicated_assignment_patterns");
        $finish;
    end
endmodule
