module tb;
    typedef logic [7:0] array_t [0:2];
    array_t result;
    logic selector, inner;
    int selectors, left_calls, right_calls;

    function automatic logic select_once(input logic value);
        selectors++;
        return value;
    endfunction
    function automatic array_t left_value();
        array_t value;
        left_calls++;
        value = '{8'h11, 8'h22, 8'h33};
        return value;
    endfunction
    function automatic array_t right_value();
        array_t value;
        right_calls++;
        value = '{8'h12, 8'h22, 8'h34};
        return value;
    endfunction

    initial begin
        selectors = 0; left_calls = 0; right_calls = 0;
        selector = 0;
        result = select_once(selector) ? left_value() : right_value();
        if (result[0] !== 8'h12 || result[2] !== 8'h34) $fatal(1, "false value");
        if (selectors != 1 || left_calls != 0 || right_calls != 1) $fatal(1, "false effects");
        selector = 1;
        result = select_once(selector) ? left_value() : right_value();
        if (result[0] !== 8'h11 || result[2] !== 8'h33) $fatal(1, "true value");
        if (selectors != 2 || left_calls != 1 || right_calls != 1) $fatal(1, "true effects");
        selector = 1'bx;
        result = select_once(selector) ? left_value() : right_value();
        if (result[0] !== 8'hxx || result[1] !== 8'h22 || result[2] !== 8'hxx)
            $fatal(1, "ambiguous value");
        if (selectors != 3 || left_calls != 2 || right_calls != 2) $fatal(1, "ambiguous effects");
        selector = 1; inner = 1;
        result = select_once(selector) ? left_value() :
                 (select_once(inner) ? right_value() : left_value());
        if (selectors != 4 || left_calls != 3 || right_calls != 2) $fatal(1, "nested skipped arm");
        selector = 0; inner = 0;
        result = select_once(selector) ? left_value() :
                 (select_once(inner) ? right_value() : left_value());
        if (selectors != 6 || left_calls != 4 || right_calls != 2) $fatal(1, "nested selected arm");
        $display("effects=%0d,%0d,%0d result=%h,%h,%h",
                 selectors, left_calls, right_calls, result[0], result[1], result[2]);
        $finish(0);
    end
endmodule
