// IEEE 1800-2009 §§7.6, 13.4: automatic fixed inputs, locals and owned return.
module tb;
    typedef logic [16:0] row_t [0:16777215];
    row_t source, target;
    function automatic row_t copy(input row_t input_value, input int index);
        row_t local_value;
        local_value = input_value;
        input_value[index] = 0;
        local_value[1] = 17'h12345;
        return local_value;
    endfunction
    initial begin
        source[0] = 17'h1abcd;
        source[16777215] = 9;
        target = copy(source, 0);
        if (source[0] !== 17'h1abcd || target[0] !== 17'h1abcd || target[1] !== 17'h12345 || target[16777215] !== 9) $fatal;
        source[0] = 7;
        target = copy(source, 16777215);
        if (source[16777215] !== 9 || target[0] !== 7 || target[16777215] !== 9 || target[2] !== 17'bx) $fatal;
        $display("PASS rtl002 value call");
        $finish(0);
    end
endmodule
