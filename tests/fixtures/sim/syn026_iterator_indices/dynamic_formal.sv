// IEEE 1800-2009 7.12.3-7.12.4: evaluate one map per element and return
// declared signed int coordinates for a live dimension selected at runtime.
module tb;
    typedef int array_t [2:4];
    array_t source;
    int receiver_calls, dimension_calls, map_calls;
    int mapped_total, formal_total, index_width_total;

    function automatic array_t make_values();
        array_t values;
        receiver_calls++;
        values[2] = 10; values[3] = 20; values[4] = 30;
        return values;
    endfunction

    function automatic int select_dimension();
        dimension_calls++;
        return 1;
    endfunction

    function automatic int mapped(input int value, input int coordinate);
        map_calls++;
        return value + coordinate;
    endfunction

    function automatic int fold_formal(input array_t values, input int dimension);
        return values.sum(item) with (item.index(dimension));
    endfunction

    initial begin
        source[2] = 10; source[3] = 20; source[4] = 30;
        mapped_total = make_values().sum(item) with
            (mapped(item, item.index(select_dimension())));
        formal_total = fold_formal(source, select_dimension());
        index_width_total = source.sum(item) with ($bits(item.index()));
        $display("mapped=%0d receiver=%0d dimension=%0d map=%0d formal=%0d width=%0d",
                 mapped_total, receiver_calls, dimension_calls, map_calls,
                 formal_total, index_width_total);
        $finish(0);
    end
endmodule
