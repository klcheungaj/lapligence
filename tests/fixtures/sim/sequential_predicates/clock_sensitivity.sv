module tb;
    logic clk, enable, ready, decision;
    logic [7:0] a, b, combinational, registered;
    function automatic logic last_clause();
        return decision;
    endfunction
    always_comb combinational = enable &&& ready &&& last_clause() ? a : b;
    always_ff @(posedge clk)
        if (enable &&& ready &&& last_clause()) registered <= a;
        else registered <= b;
    initial begin
        clk = 0; enable = 1; ready = 1; decision = 0; a = 8'ha5; b = 8'ha6;
        #1;
        if (combinational !== 8'ha6) $fatal(1, "initial sensitivity");
        decision = 1; #1;
        if (combinational !== 8'ha5) $fatal(1, "last clause function dependency");
        clk = 1; #1;
        if (registered !== 8'ha5) $fatal(1, "clock true");
        clk = 0; ready = 0; #1;
        if (combinational !== 8'ha6) $fatal(1, "middle clause dependency");
        enable = 1'bx; #1;
        if (combinational !== 8'b101001xx) $fatal(1, "X before false");
        clk = 1; #1;
        if (registered !== 8'ha6) $fatal(1, "ambiguous if takes else");
        $display("clock_sensitivity=pass");
        $finish(0);
    end
endmodule
