// SV 10.9.1 / 7.6: an array-valued positional item supplies one element per
// cell of its subarray, left bound to left bound; it is not replicated.
module tb;
    typedef logic signed [6:0] lane_t;
    typedef lane_t row_t [1:0];
    localparam row_t ROW_P = '{lane_t'(8), lane_t'(9)};
    row_t stored, other;
    lane_t same [1:0][1:0];
    lane_t negative [1:0][-1:0];
    lane_t ascending [0:1][0:1];
    lane_t cube [1:0][1:0][1:0];
    lane_t initialized [1:0][1:0] = '{ROW_P, '{lane_t'(10), lane_t'(11)}};
    integer calls;

    function automatic row_t make_row(input lane_t a, input lane_t b);
        calls++;
        return '{a, b};
    endfunction

    initial begin
        calls = 0;
        stored = '{lane_t'(-1), lane_t'(2)};
        other = '{lane_t'(1), lane_t'(3)};
        if (initialized[1][1] !== 8 || initialized[1][0] !== 9 ||
            initialized[0][1] !== 10 || initialized[0][0] !== 11)
            $fatal(1, "declaration initializer row item");
        same = '{stored, other};
        if (same[1][1] !== -1 || same[1][0] !== 2 || same[0][1] !== 1 || same[0][0] !== 3)
            $fatal(1, "row items in a descending target");
        negative = '{stored, other};
        if (negative[1][-1] !== -1 || negative[1][0] !== 2 ||
            negative[0][-1] !== 1 || negative[0][0] !== 3)
            $fatal(1, "row items keep left-to-left correspondence");
        ascending = '{stored, other};
        if (ascending[0][0] !== -1 || ascending[0][1] !== 2 ||
            ascending[1][0] !== 1 || ascending[1][1] !== 3)
            $fatal(1, "row items in an ascending target");
        same = '{default: stored};
        if (same[1][1] !== -1 || same[1][0] !== 2 || same[0][1] !== -1 || same[0][0] !== 2)
            $fatal(1, "array-valued default reaches each row");
        same = '{default: 5};
        if (same[1][1] !== 5 || same[1][0] !== 5 || same[0][1] !== 5 || same[0][0] !== 5)
            $fatal(1, "scalar default fills every cell");
        same = '{make_row(4, 5), other};
        if (same[1][1] !== 4 || same[1][0] !== 5 || same[0][1] !== 1 || same[0][0] !== 3 ||
            calls != 1)
            $fatal(1, "call row item evaluates once");
        cube = '{same, '{other, stored}};
        if (cube[1][1][1] !== 4 || cube[1][1][0] !== 5 || cube[1][0][1] !== 1 ||
            cube[1][0][0] !== 3 || cube[0][1][1] !== 1 || cube[0][1][0] !== 3 ||
            cube[0][0][1] !== -1 || cube[0][0][0] !== 2)
            $fatal(1, "matrix and nested row items");
        same <= '{other, stored};
        #1;
        if (same[1][1] !== 1 || same[1][0] !== 3 || same[0][1] !== -1 || same[0][0] !== 2)
            $fatal(1, "nonblocking row items");
        $display("NESTED_ROW_PATTERNS_PASS");
        $finish(0);
    end
endmodule
