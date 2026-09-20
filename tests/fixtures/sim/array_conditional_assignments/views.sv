module tb;
    typedef logic [7:0] asc_t [-2:-1];
    typedef logic [7:0] desc_t [5:4];
    asc_t a, b;
    desc_t result;
    logic [7:0] rows [1:0][-2:-1];
    logic selector;
    int source_calls, target_calls, source_row;
    function automatic int select_source();
        source_calls++;
        return source_row;
    endfunction
    function automatic int select_target();
        target_calls++;
        return 0;
    endfunction
    initial begin
        a = '{8'h11, 8'h22}; b = '{8'h33, 8'h44};
        selector = 1;
        result = selector ? a : b;
        if (result[5] !== 8'h11 || result[4] !== 8'h22) $fatal(1, "left to right correspondence");
        rows[1] = a; rows[0] = b;
        source_calls = 0; target_calls = 0; source_row = 1;
        rows[select_target()] = selector ? rows[select_source()] : b;
        if (source_calls != 1 || target_calls != 1) $fatal(1, "selected indices evaluated more than once");
        if (rows[0][-2] !== 8'h11 || rows[0][-1] !== 8'h22) $fatal(1, "selected row copy");
        selector = 0;
        rows[select_target()] = selector ? rows[select_source()] : b;
        if (source_calls != 1 || target_calls != 2) $fatal(1, "unselected index evaluated");
        source_row = 3; selector = 1;
        result = selector ? rows[select_source()] : a;
        if (result[5] !== 8'hxx || result[4] !== 8'hxx) $fatal(1, "out of range source row");
        $display("views=%h,%h calls=%0d,%0d", rows[0][-2], rows[0][-1], source_calls, target_calls);
        $finish(0);
    end
endmodule
