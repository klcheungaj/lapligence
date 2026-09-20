module tb;
    typedef logic [7:0] array_t [2:0];
    array_t source;
    int calls, result, folded, local_result, skipped;
    function automatic array_t make(input int base);
        array_t values;
        calls++;
        values[2] = base; values[1] = base + 1; values[0] = base + 2;
        return values;
    endfunction
    function automatic int fold(input array_t values, input int bias);
        int scale;
        scale = 2;
        return values.sum() with (int'(item) * scale + bias);
    endfunction
    function automatic int local_fold(input array_t values);
        array_t local_values;
        int offset;
        local_values = values;
        offset = 10;
        return local_values.sum(v) with (int'(v) + offset);
    endfunction
    initial begin
        calls = 0;
        source[2] = 1; source[1] = 2; source[0] = 3;
        result = make(4).sum();
        folded = fold(source, 5);
        local_result = local_fold(source);
        skipped = 0 ? make(100).sum() : 7;
        $display("receiver=%0d calls=%0d captured=%0d local=%0d skipped=%0d",
                 result, calls, folded, local_result, skipped);
        $finish(0);
    end
endmodule
