// llg-test-fixture: tests/fixtures/sim/data_types_next/syn_005_inside_array_values.sv
// IEEE 1800-2009 §11.4.13: an unpacked fixed-array set item contributes its
// elements, while a packed value remains one set item.
module tb;
    typedef logic [7:0] lane_t [0:1];
    typedef logic [3:0] matrix_t [0:1][1:0];

    lane_t stored;
    lane_t left_value;
    lane_t right_value;
    matrix_t stored_matrix;
    logic [15:0] packed_value;
    logic [7:0] value;
    logic selector;
    logic result;
    integer calls;

    function automatic lane_t make_lane(input logic [7:0] first,
                                         input logic [7:0] second);
        calls = calls + 1;
        make_lane = '{first, second};
    endfunction

    function automatic matrix_t make_matrix();
        make_matrix = '{'{4'h1, 4'h2}, '{4'ha, 4'hb}};
    endfunction

    initial begin
        calls = 0;
        stored = '{8'h11, 8'h22};
        left_value = '{8'h33, 8'h44};
        right_value = '{8'h55, 8'h66};
        stored_matrix = '{'{4'h1, 4'h2}, '{4'ha, 4'hb}};
        packed_value = 16'h1122;

        // Existing storage-array expansion is the positive control.
        value = 8'h22;
        result = value inside {stored};
        if (result !== 1'b1) $fatal(1, "storage array membership");

        // A packed vector with the same payload is one set member.
        result = value inside {packed_value};
        if (result !== 1'b0) $fatal(1, "packed value must remain singular");

        // Function-returned arrays and array-valued conditionals are expanded.
        result = value inside {make_lane(8'h11, 8'h22)};
        if (result !== 1'b1 || calls != 1)
            $fatal(1, "function array membership or one-call capture");

        selector = 1'b0;
        result = 8'h66 inside {selector ? left_value : right_value};
        if (result !== 1'b1) $fatal(1, "conditional array membership");

        // A nested fixed array is recursively visited to its packed leaves.
        result = 4'hb inside {make_matrix()};
        if (result !== 1'b1) $fatal(1, "nested array membership");

        // A selected row keeps the remaining fixed-array dimensions.
        result = 4'h2 inside {stored_matrix[0]};
        if (result !== 1'b1) $fatal(1, "selected row membership");

        // A fixed-array cast remains an array value for membership expansion.
        result = 8'h22 inside {lane_t'(16'h1122)};
        if (result !== 1'b1) $fatal(1, "casted array membership");

        // Wildcard RHS elements and a later definite match keep LRM dominance.
        value = 8'hx5;
        result = value inside {make_lane(8'h05, 8'h?5)};
        if (result !== 1'b1 || calls != 2)
            $fatal(1, "wildcard or known-match membership");

        $display("PASS syn_005_inside_array_values");
        $finish(0);
    end
endmodule
