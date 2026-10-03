// llg-test-fixture: SYN-003 procedural destination coordinates before scatter.
// IEEE 1800-2009 10.9; no ordering is assumed between RHS and LHS evaluation.
module tb;
    typedef int pair_t[2];
    typedef int matrix_t[2][2];
    typedef struct packed { int first; int second; } packed_pair_t;
    typedef struct { int first; int second; } record_pair_t;
    pair_t source;
    matrix_t nested;
    packed_pair_t packed_source;
    record_pair_t record_source;
    int index, row, column, rhs_calls, selector_calls;
    int memory[2], grid[2][2];
    int a, b, c, d;

    function automatic pair_t rhs_value();
        rhs_calls++;
        return source;
    endfunction
    function automatic int selected_index();
        selector_calls++;
        return index;
    endfunction
    function automatic int local_capture();
        pair_t local_memory;
        int local_index;
        local_index = 0;
        local_memory = '{0, 0};
        pair_t'{local_index, local_memory[local_index]} = source;
        if (local_index != 1 || local_memory[0] != 9 || local_memory[1] != 0)
            return 0;
        return 1;
    endfunction

    initial begin
        rhs_calls = 0;
        selector_calls = 0;
        source = '{1, 9};
        index = 0;
        memory = '{0, 0};
        pair_t'{index, memory[selected_index()]} = rhs_value();
        if (index != 1 || memory[0] != 9 || memory[1] != 0 ||
            rhs_calls != 1 || selector_calls != 1) $fatal(1, "array target capture");
        if (local_capture() != 1) $fatal(1, "activation target capture");

        index = 0;
        memory = '{0, 0};
        packed_source = '{1, 17};
        packed_pair_t'{index, memory[index]} = packed_source;
        if (index != 1 || memory[0] != 17 || memory[1] != 0)
            $fatal(1, "packed record target capture");
        index = 0;
        memory = '{0, 0};
        record_source = '{1, 23};
        record_pair_t'{index, memory[index]} = record_source;
        if (index != 1 || memory[0] != 23 || memory[1] != 0)
            $fatal(1, "unpacked record target capture");

        row = 0;
        column = 0;
        grid = '{default:'{default:0}};
        nested = '{'{1, 1}, '{31, 41}};
        '{'{row, column}, '{grid[row][column], memory[0]}} = nested;
        if (row != 1 || column != 1 || grid[0][0] != 31 ||
            grid[1][1] != 0 || memory[0] != 41) $fatal(1, "nested targets");

        a = 1; b = 2; c = 3; d = 4;
        '{'{a, b}, '{c, d}} = matrix_t'{'{d, c}, '{b, a}};
        if (a != 4 || b != 3 || c != 2 || d != 1) $fatal(1, "RHS snapshot");

        index = 0;
        memory = '{0, 0};
        source = '{1, 55};
        pair_t'{index, memory[selected_index()]} <= rhs_value();
        source = '{0, 0};
        index = 1;
        if (memory[0] != 0 || memory[1] != 0) $fatal(1, "NBA early publication");
        #1;
        if (index != 1 || memory[0] != 55 || memory[1] != 0 ||
            rhs_calls != 2 || selector_calls != 2) $fatal(1, "NBA capture");
        $display("PATTERN_CAPTURE_PASS");
        $finish(0);
    end
endmodule
