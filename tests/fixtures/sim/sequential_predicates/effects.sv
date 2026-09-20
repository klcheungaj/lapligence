module tb;
    int trace, left_calls, right_calls;
    function automatic logic clause(input int digit, input logic value);
        trace = trace * 10 + digit;
        return value;
    endfunction
    function automatic logic [7:0] left_value();
        left_calls++;
        return 8'ha5;
    endfunction
    function automatic logic [7:0] right_value();
        right_calls++;
        return 8'ha6;
    endfunction
    task automatic check(input logic a, b, c, input int wanted_trace,
                         input int wanted_left, wanted_right, input logic [7:0] wanted);
        logic [7:0] result;
        int selected;
        trace = 0; left_calls = 0; right_calls = 0;
        result = clause(1, a) &&& clause(2, b) &&& clause(3, c) ?
                 left_value() : right_value();
        if (result !== wanted || trace != wanted_trace ||
            left_calls != wanted_left || right_calls != wanted_right)
            $fatal(1, "ternary side effects %0d %0d %0d", trace, left_calls, right_calls);
        trace = 0;
        if (clause(1, a) &&& clause(2, b) &&& clause(3, c)) selected = 1;
        else selected = 2;
        if (trace != wanted_trace || selected != ((wanted === 8'ha5) ? 1 : 2))
            $fatal(1, "if side effects");
    endtask
    initial begin
        check(1, 1, 1, 123, 1, 0, 8'ha5);
        check(1, 0, 1, 12, 0, 1, 8'ha6);
        check(1'bx, 0, 1, 1, 1, 1, 8'b101001xx);
        check(1, 1'bz, 0, 12, 1, 1, 8'b101001xx);
        check(1, 1, 1'bx, 123, 1, 1, 8'b101001xx);
        check(0, 1'bx, 1'bz, 1, 0, 1, 8'ha6);
        $display("effects=6 cases passed");
        $finish(0);
    end
endmodule
