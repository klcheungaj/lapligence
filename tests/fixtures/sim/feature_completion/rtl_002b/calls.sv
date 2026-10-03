// IEEE 1800-2009 §§7.6, 13.5: descriptor formals preserve passing modes and lifetimes.
module tb;
    typedef logic [16:0] row_t [0:16777215];
    row_t source, target, output_value;
    function automatic int inspect(input row_t value);
        value[0] = value[0] + 1;
        return value[0];
    endfunction
    function row_t persistent(input row_t value, input bit write);
        row_t saved;
        if (write) saved = value;
        return saved;
    endfunction
    function automatic row_t descend(input row_t value, input int depth);
        row_t local_value;
        local_value = value;
        local_value[depth] = depth + 10;
        if (depth > 0) local_value = descend(local_value, depth - 1);
        return local_value;
    endfunction
    function automatic void update(output row_t out_value, inout row_t in_value, ref row_t ref_value);
        if (out_value[0] !== 17'bx) $fatal;
        out_value = in_value;
        in_value[0] = 21;
        ref_value[16777215] = 22;
    endfunction
    initial begin
        source[0] = 7;
        source[16777215] = 9;
        if (inspect(source) != 8 || source[0] !== 7) $fatal;
        target = persistent(source, 1);
        source[0] = 8;
        target = persistent(source, 0);
        if (target[0] !== 7 || target[16777215] !== 9) $fatal;
        target = descend(source, 3);
        if (target[0] !== 10 || target[1] !== 11 || target[2] !== 12 || target[3] !== 13 || source[0] !== 8) $fatal;
        update(output_value, target, source);
        if (output_value[0] !== 10 || target[0] !== 21 || source[16777215] !== 22) $fatal;
        $display("PASS rtl002b calls");
        $finish(0);
    end
endmodule
