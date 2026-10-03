// IEEE 1800-2009 §§10.9, 10.10, 6.8: oversized declaration initializers,
// array-valued pattern items and pattern-lvalue scatter through descriptor rows.
`ifndef RTL002B_COUNT
`define RTL002B_COUNT 65537
`endif
module tb;
    localparam int N = `RTL002B_COUNT;
    // Two rows must fit the unpacked element budget at the 16M witness.
    localparam int R = N > 65537 ? N / 2 : N;
    typedef logic [16:0] row_t [0:R-1];
    logic [16:0] initialized [0:N-1] = '{5: 17'h2, default: 17'h1};
    bit [16:0] two_state [0:N-1] = '{default: 17'h3};
    row_t left_value, right_value, x, y;
    row_t pair [0:1];
    row_t descending [1:0];
    int calls;
    function automatic row_t counted(input row_t value);
        calls = calls + 1;
        return value;
    endfunction
    initial begin
        if (initialized[0] !== 17'h1 || initialized[5] !== 17'h2 || initialized[N-1] !== 17'h1) $fatal(1, "declaration");
        if (two_state[N-1] !== 17'h3) $fatal(1, "two-state declaration");
        left_value = '{default: 17'h3};
        right_value = '{default: 17'h4};
        left_value[7] = 'z;
        pair = '{left_value, right_value};
        if (pair[0][7] !== 17'bz || pair[0][0] !== 17'h3 || pair[1][R-1] !== 17'h4) $fatal(1, "items");
        pair = '{default: right_value};
        if (pair[0][7] !== 17'h4 || pair[1][0] !== 17'h4) $fatal(1, "default item");
        calls = 0;
        pair = '{0: counted(left_value), 1: right_value};
        if (calls != 1 || pair[0][7] !== 17'bz || pair[1][7] !== 17'h4) $fatal(1, "call item");
        pair = '{pair[1], pair[0]};
        if (pair[0][7] !== 17'h4 || pair[1][7] !== 17'bz) $fatal(1, "overlapping items");
        '{x, y} = pair;
        if (x[7] !== 17'h4 || y[7] !== 17'bz || y[R-1] !== 17'h3) $fatal(1, "scatter");
        '{pair[1], pair[0]} = pair;
        if (pair[0][7] !== 17'bz || pair[1][7] !== 17'h4) $fatal(1, "overlapping scatter");
        descending = pair;
        '{x, y} = descending;
        if (x[7] !== 17'bz || y[7] !== 17'h4) $fatal(1, "descending scatter");
        '{x, y} <= pair;
        pair[0][7] = 17'h9;
        #1;
        if (x[7] !== 17'bz || pair[0][7] !== 17'h9) $fatal(1, "nonblocking scatter");
        $display("PASS rtl002b patterns");
        $finish(0);
    end
endmodule
