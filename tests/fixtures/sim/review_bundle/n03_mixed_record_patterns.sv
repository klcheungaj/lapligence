// SV 10.9.1: explicit indices override recursive type setters in every context.
module tb;
    typedef struct { int x; int y; } record_t;
    typedef record_t records_t[1:-1];
    localparam int KEY = 0;
    records_t values;
    record_t special;
    int seed;

    function automatic records_t make(input record_t explicit_value, input int fill);
        records_t local_values = '{(KEY):explicit_value, int:fill};
        return local_values;
    endfunction
    function automatic records_t make_return(input record_t explicit_value, input int fill);
        return '{(KEY):explicit_value, int:fill};
    endfunction
    function automatic bit correct(input records_t actual, input int fill);
        return actual[1].x == fill && actual[1].y == fill &&
               actual[0].x == 11 && actual[0].y == 22 &&
               actual[-1].x == fill && actual[-1].y == fill;
    endfunction

    initial begin
        special = '{11, 22};
        seed = 7;
        values = '{(KEY):special, int:seed};
        if (!correct(values, 7)) $fatal(1, "module record recursion/order");
        values = make(special, seed + 1);
        if (!correct(values, 8)) $fatal(1, "automatic declaration/call");
        values = make_return(special, seed + 2);
        if (!correct(values, 9)) $fatal(1, "return pattern");
        if (!correct('{(KEY):special, int:seed + 3}, 10))
            $fatal(1, "input argument pattern");
        values <= '{(KEY):special, int:seed};
        seed = 99;
        special = '{33, 44};
        #1;
        if (!correct(values, 7)) $fatal(1, "NBA captured pattern values");
        $display("PASS n03_mixed_record_patterns");
        $finish(0);
    end
endmodule
