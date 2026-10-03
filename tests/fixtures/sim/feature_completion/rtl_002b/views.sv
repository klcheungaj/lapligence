// IEEE 1800-2009 §§7.6, 11.4.11, 13.5: selected copies, refs and conditional elements.
module tb;
    typedef logic [16:0] row_t [0:65536];
    typedef row_t rows_t [0:1];
    rows_t rows;
    row_t left_value, right_value, result;
    int index, calls;
    logic choose;
    function automatic row_t counted(input row_t value, input int amount);
        calls = calls + amount;
        return value;
    endfunction
    function automatic void change(ref row_t value);
        value[65536] = 77;
    endfunction
    initial begin
        left_value = '{default:17'h12345};
        right_value = left_value;
        left_value[0] = 1;
        right_value[0] = 2;
        left_value[1] = 'z;
        right_value[1] = 'z;
        rows[0] = left_value;
        rows[1] = right_value;
        index = 0;
        result = rows[index++];
        if (index != 1 || result[0] !== 1 || result[65536] !== 17'h12345) $fatal;
        rows[index] = rows[0];
        change(rows[index]);
        if (rows[1][65536] !== 77 || rows[0][65536] !== 17'h12345) $fatal;
        if (rows[0] !== left_value) $fatal;
        choose = 1;
        result = choose ? counted(left_value, 1) : counted(right_value, 10);
        if (calls != 1 || result[0] !== 1) $fatal;
        choose = 'x;
        result = choose ? counted(left_value, 1) : counted(right_value, 10);
        if (calls != 12 || result[0] !== 'x || result[1] !== 'x || result[2] !== 17'h12345) $fatal;
        index = 2;
        result = rows[index];
        if (result[0] !== 'x || result[65536] !== 'x) $fatal;
        index = 0;
        rows[index] <= rows[1];
        rows[1][65536] = 88;
        index = 1;
        #1;
        if (rows[0][65536] !== 77 || rows[1][65536] !== 88) $fatal;
        $display("PASS rtl002b views");
        $finish(0);
    end
endmodule
